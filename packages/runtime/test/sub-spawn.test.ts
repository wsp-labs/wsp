// SPDX-License-Identifier: AGPL-3.0-only
// A lead thread on this computer starting a sub-thread on Boat: every check
// that can refuse runs before Boat is asked for a start, the machine forks the
// image head at the size asked or the cloud's own, the picked files land in
// /root/<repo> through the file road, and the thread opens there with the
// preamble. A machine that fails before the first turn is deleted and replaced
// once. The lead's folder is a real repo here; Boat is a stub backend that
// keeps every create, command and file, and the lead's daemon a fake that keeps
// every frame.
import { describe, expect, it } from "vitest";
import { BOX_WORK_DIR } from "@wsp/engine";
import {
  agentsOffRefusal, SUB_SEED_MAX_BYTES, exitClassOf, fmtBytes, refusalLine, spawnCapRefusal, subAheadRefusal, subBranchNameRefusal, subDayStartsRefusal, subDetachedRefusal,
  subHourStartsRefusal, subLeadBranchRefusal, subMachineTakenRefusal, subNoAgentRefusal, subNoCloudRefusal, subNoImageRefusal, subNoRemoteRefusal, subNoSignInRefusal, subNoThreadRefusal,
  subNoUpstreamRefusal, subOpening, subOtherHostRefusal, subSeedTooBigRefusal, subSizeRefusal, spendCapRefusal, type Caller,
} from "@wsp/protocol";
import type { Runtime } from "../src/runtime.js";
import { PAID, TRIAL, leadOnThisComputer, type Boat, type Fault, type Setup } from "./sub-fixture.js";

const TASK = "Fix the flaky upload test in packages/upload.";
/** What every fork gets from the create road, none of it carrying a secret: its hostname, and the machine context
 * (the probe, then the archive of the wsp-machine skill and its facts landed by the file road and unpacked). */
const CONTEXT_ARCHIVE = "/tmp/wsp-vault-in-";
const forkRoad = (cmd: string): boolean => cmd.startsWith("hostname ") || cmd.includes("WSP_CTX") || cmd.includes(CONTEXT_ARCHIVE);
const seedFiles = (boat: Boat): string[] => boat.files.map(f => f.path).filter(p => !p.startsWith(CONTEXT_ARCHIVE));
const NAME = "Fix the flaky upload test";
const spawn = (rt: Runtime, project: string, lead: Caller | undefined, sub: { on?: string; size?: string; branch?: string; harness?: string } = {}, name = NAME) =>
  rt.workspaces.create({ project, name, sub: { on: "boat", ...sub } }, lead);

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
  it("forks the image head at the cloud's size, lands the picked files in /root/<repo> by the file road, and opens the sub-thread there under the preamble", async () => {
    const { rt, store, boat, daemon, starts, project, threadId, lead, token, folder } = await leadOnThisComputer({ picks: [".env", ".env.local"], git: { uncommitted: 2 } });
    const before = daemon.frames.length;
    const made = await spawn(rt, project.id, lead);
    expect(boat.creates).toHaveLength(1);
    expect(boat.creates[0]).toMatchObject({ fromSnapshot: "wsp-host-default-v1", cpu: 2, memMb: 4096 });
    expect(made.sub!.out).toEqual({
      on: "boat",
      machine: NAME,
      size: { cpu: 2, memMb: 4096 },
      rateUsdPerHour: 0.018,
      repo: "https://github.com/acme/web.git",
      base: { branch: "feat/upload", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" },
      branch: "wsp/fix-the-flaky-upload-test",
      sent: { paths: [".env", ".env.local"], bytes: 1243, missing: [] },
      notSent: { uncommitted: 2 },
      deleteAfterMinutes: 30,
    });
    expect(made.sub!.folder).toBe("/root/web");
    expect(made.notice).toBe("2 uncommitted changes in web did not travel; the sub-thread starts at feat/upload as pushed (4be1c0a).");
    // The preamble, word for word as 03 gives it.
    expect(made.sub!.preamble).toBe(
      [
        `This thread runs on a machine of its own on Boat (small, 2 vCPU, 4 GB), started by thread ${threadId.slice(0, 8)}. The code is not here yet. The folder you are in, /root/web, holds only the git-ignored files the person picked: .env, .env.local. Get the code first, in this folder:`,
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
    // The picked files went up as one archive by the file road, and only their names are in a command.
    const machine = boat.machines[0]!;
    expect(seedFiles(boat)).toEqual([`/tmp/wsp-seed-${made.id}.tgz`]);
    expect(machine.execLog).toContain("install -d -m 0755 '/root/web'");
    expect(machine.execLog).toContain(`tar -xzf '/tmp/wsp-seed-${made.id}.tgz' -C '/root/web' && rm -f '/tmp/wsp-seed-${made.id}.tgz'`);
    // The record names the lead's project and the lead, and stands in its root's tree.
    const record = (await store.get("workspaces", made.id)) as Record<string, unknown>;
    expect(record).toMatchObject({ kind: "cloud", project: project.id, lead: { threadId, workspaceId: folder.id }, rootThreadId: threadId, parentThreadId: threadId, agents: { spawn: false }, golden: "wsp-host-default-v1", size: { cpu: 2, memMb: 4096 } });
    expect(record["sub"]).toEqual({ on: "boat", repo: "https://github.com/acme/web.git", base: { branch: "feat/upload", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" }, branch: "wsp/fix-the-flaky-upload-test", folder: "/root/web" });
    // The thread opens in that folder, the preamble above the task, and is the lead's child.
    const child = await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: made.sub!.harness, notify: ["me"], turnToken: token, title: NAME, cwd: made.sub!.folder }, lead);
    await child.finished;
    const opened = starts.at(-1)!.o;
    expect(opened.cwd).toBe("/root/web");
    expect(opened.prompt).toBe(`${made.sub!.preamble}\n\n${TASK}`);
    expect((await rt.sessions.list()).find(r => r.threadId === child.view().threadId)).toMatchObject({ parentThreadId: threadId, rootThreadId: threadId, workspaceId: made.id });
    // wsp asked the lead's daemon one read and pushed nothing.
    expect(daemon.frames.slice(before).map(f => f["op"])).toEqual(["git.status"]);
  });

  it("sends nothing ignored without the person's picks, says why, and names a picked path no longer there", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer();
    const made = await spawn(rt, project.id, lead);
    expect(made.sub!.out.sent).toEqual({ paths: [], bytes: 0, missing: [] });
    expect(made.sub!.out.notSent).toEqual({ uncommitted: 0, why: "nothing is picked for web" });
    expect(made.notice).toBe("no git-ignored files were sent: nothing is picked for web. wsp projects seed web shows what could go.");
    expect(made.sub!.preamble).toContain("The folder you are in, /root/web, is empty.");
    expect(seedFiles(boat)).toEqual([]);
    expect(boat.machines[0]!.execLog.some(c => c.startsWith("tar "))).toBe(false);
    const again = await leadOnThisComputer({ picks: [".env", "gone.json"] });
    const sent = await spawn(again.rt, again.project.id, again.lead);
    expect(sent.sub!.out.sent).toEqual({ paths: [".env"], bytes: 1100, missing: ["gone.json"] });
  });

  it("takes the size the lead names, at its class's rate, and numbers the default branch past one another sub-thread pushes", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer({ agents: { spawn: true, maxMachines: 5 } });
    const big = await spawn(rt, project.id, lead, { size: "4x8" });
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

  it("refuses a size the plan does not sell, naming the ones it does, before any create", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer();
    boat.limitsRead = TRIAL;
    const big = await refused(spawn(rt, project.id, lead, { size: "8x16" }));
    expect(big).toEqual({ said: subSizeRefusal("Boat", "8x16", TRIAL.sizes).message, cls: "usage" });
    expect(big.said).toBe("Boat sells small 2x4 ($0.018/h) and default 4x8 ($0.036/h) on this plan; 8x16 is not one of them. Name one of them, or leave the size out for the cloud's own.");
    const odd = await refused(spawn(rt, project.id, lead, { size: "3x5" }));
    expect(odd).toEqual({ said: subSizeRefusal("Boat", "3x5", TRIAL.sizes).message, cls: "usage" });
    expect(boat.creates).toEqual([]);
  });

  it("reads the lead's own machine for its place, counts the sub-machine against the root, and runs no second thread on it", async () => {
    const { rt, boat, project, lead, threadId, token } = await leadOnThisComputer({ agents: { spawn: true, maxMachines: 1 } });
    const made = await spawn(rt, project.id, lead);
    // checkoutOf: the folder a turn and a command start in is the sub-machine's own.
    expect(made.folder).toBe("/root/web");
    // The root's machines count it: a second spawn meets the cap before any create.
    const capped = await refused(spawn(rt, project.id, lead, {}, "Another one"));
    expect(capped).toEqual({ said: spawnCapRefusal(threadId, 1, 1), cls: "usage" });
    expect(boat.creates).toHaveLength(1);
    const child = await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: "claude", turnToken: token, cwd: made.sub!.folder }, lead);
    await child.finished;
    const childId = child.view().threadId!;
    // placeOfThread: the sub-thread's own refusals name Boat, the place it runs at, not the lead's computer.
    const asChild: Caller = { origin: "relayed", by: { kind: "thread", threadId: childId, workspaceId: made.id, rootThreadId: threadId } };
    const second = await refused(rt.sessions.start(made.id, { prompt: "beside it", harness: "claude" }, lead));
    expect(second).toEqual({ said: subMachineTakenRefusal(NAME, childId).message, cls: "usage" });
    // A sub-thread starts nothing of its own: its machine's switch is off.
    const grand = await refused(rt.workspaces.create({ project: project.id, name: "grandchild", sub: { on: "boat" } }, asChild));
    expect(grand).toEqual({ said: agentsOffRefusal(NAME, "fork"), cls: "usage" });
    expect(boat.creates).toHaveLength(1);
  });
});

describe("every check refuses before Boat is asked for a start", () => {
  const cases: { name: string; setup?: Setup; sub?: { on?: string; size?: string; branch?: string; harness?: string }; asked?: "terminal"; expect: (f: Awaited<ReturnType<typeof leadOnThisComputer>>) => { said: string; cls: string }; before?: (f: Awaited<ReturnType<typeof leadOnThisComputer>>) => void | Promise<void> }[] = [
    { name: "S1 a line from no thread", asked: "terminal", expect: () => ({ said: subNoThreadRefusal("boat").message, cls: "usage" }) },
    { name: "S2 the lead's switch off", setup: { agents: { spawn: false } }, expect: () => ({ said: agentsOffRefusal("web", "fork"), cls: "usage" }) },
    { name: "S3 a cloud this host holds no key for", sub: { on: "solari" }, expect: () => ({ said: subNoCloudRefusal("solari", ["Boat"]).message, cls: "usage" }) },
    { name: "S4 a folder with no remote", setup: { remote: null }, expect: () => ({ said: subNoRemoteRefusal("web", "Boat").message, cls: "usage" }) },
    { name: "S4 a remote on another host", setup: { remote: "https://git.example.com/acme/web.git" }, expect: () => ({ said: subOtherHostRefusal("web", "git.example.com", "Boat").message, cls: "usage" }) },
    { name: "S5 a lead on no branch", setup: { git: { head: "(detached)" } }, expect: () => ({ said: subDetachedRefusal("web").message, cls: "usage" }) },
    { name: "S5 a branch the remote lacks", setup: { git: { upstream: undefined } }, expect: () => ({ said: subNoUpstreamRefusal("feat/upload").message, cls: "usage" }) },
    { name: "S5 commits the remote lacks", setup: { git: { ahead: 2 } }, expect: () => ({ said: subAheadRefusal("feat/upload", 2).message, cls: "usage" }) },
    { name: "S6 no image on Boat", setup: { image: false }, expect: () => ({ said: subNoImageRefusal("Boat").message, cls: "usage" }) },
    { name: "S6 an image with no GitHub sign-in", setup: { image: { logins: [{ name: "GitHub CLI login", state: "skipped" }] } }, expect: () => ({ said: subNoSignInRefusal("Boat", { label: "GitHub", cli: "gh", login: "GitHub CLI login" }, "acme/web").message, cls: "usage" }) },
    { name: "S7 a size the plan does not sell", sub: { size: "16x32" }, expect: () => ({ said: subSizeRefusal("Boat", "16x32", PAID.sizes).message, cls: "usage" }) },
    { name: "S8 a name git refuses", sub: { branch: "wsp/bad..name" }, expect: () => ({ said: subBranchNameRefusal("wsp/bad..name").message, cls: "usage" }) },
    { name: "S8 the lead's own branch", sub: { branch: "feat/upload" }, expect: () => ({ said: subLeadBranchRefusal("feat/upload").message, cls: "usage" }) },
    // Today's line, which names the row as the place door does; C2 makes it say Boat.
    { name: "S10 the day's spend reached", setup: { spendPerDayUsd: 0 }, expect: () => ({ said: spendCapRefusal("box", 0, 0), cls: "provider" }) },
    { name: "S11 an agent the image does not carry", setup: { image: { agents: ["codex"] } }, sub: { harness: "claude" }, expect: () => ({ said: subNoAgentRefusal("Boat", "claude", ["codex"]).message, cls: "usage" }) },
    {
      name: "S12 picks past what a sub-thread carries",
      setup: { picks: ["data.bin", ".env"], sizes: { "data.bin": SUB_SEED_MAX_BYTES + 1024, ".env": 100 } },
      expect: () => ({ said: "", cls: "usage" }),
    },
    {
      name: "S13 the hour's starts used",
      before: f => void (f.boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, hour: { limit: 60, used: 60, remaining: 0 } } }),
      expect: () => ({ said: subHourStartsRefusal("Boat", 60, 60, "the $20 plan", NAME).message, cls: "provider" }),
    },
    {
      name: "S13 the day's starts used",
      before: f => void (f.boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, day: { limit: 200, used: 200, remaining: 0 } } }),
      expect: () => ({ said: subDayStartsRefusal("Boat", 200, 200, "the $20 plan", NAME).message, cls: "provider" }),
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

  it("S12 names the largest picks and the sizes", async () => {
    const big = SUB_SEED_MAX_BYTES + 1024;
    const f = await leadOnThisComputer({ picks: ["data.bin", ".env"], sizes: { "data.bin": big, ".env": 100 } });
    const got = await refused(spawn(f.rt, f.project.id, f.lead));
    expect(got.cls).toBe("usage");
    expect(got.said).toMatch(new RegExp(`^the files picked for web come to [^,]+, over the ${fmtBytes(SUB_SEED_MAX_BYTES).replace(".", "\\.")} a sub-thread carries; the largest are data\\.bin \\(${fmtBytes(big).replace(".", "\\.")}\\), \\.env \\(100 B\\)\\. Pick again with wsp projects seed web\\.$`));
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
      expect(boat.creates).toHaveLength(2);
      const [bad, good] = boat.machines;
      expect(bad!.killed).toBe(true);
      expect(good!.killed).toBe(false);
      expect(made.machineId).toBe(good!.id);
      const line =
        fault.kind === "unusable"
          ? `Boat gave ${bad!.id} with nothing able to run on it (${BOX_WORK_DIR} is missing); wsp deleted it and started another.`
          : fault.kind === "env"
            ? `Boat's machine ${bad!.id} did not take wsp's setup within 2 minutes (exit 124); wsp deleted it and started another.`
            : `Boat could not get a machine for ${NAME} and cancelled the create (no capacity); wsp started another.`;
      expect(made.notice!.split("\n")[0]).toBe(line);
    });
  }

  it("fails the spawn on a second failure, naming both machines, and leaves neither standing", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer();
    boat.faults = [{ kind: "unusable", replayed: true }, { kind: "unusable" }];
    const got = await refused(spawn(rt, project.id, lead));
    expect(got.cls).toBe("provider");
    expect(got.said).toBe(`Boat gave two machines in a row that could not run commands (${boat.machines[0]!.id}, ${boat.machines[1]!.id}); Boat is restoring badly right now. Start it again in a few minutes.`);
    expect(boat.machines.every(m => m.killed)).toBe(true);
    expect((await rt.workspaces.list()).map(w => w.name)).not.toContain(NAME);
  });

  it("names both faults when they differ", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer();
    boat.faults = [{ kind: "env" }, { kind: "cancelled" }];
    const got = await refused(spawn(rt, project.id, lead));
    expect(got.said).toBe(`Boat gave two machines in a row that failed before the sub-thread started (${boat.machines[0]!.id}: setup not taken, exit 124; ${boat.machines[1]!.id}: no capacity). Start it again in a few minutes.`);
    expect(boat.machines.every(m => m.killed)).toBe(true);
  });
});

describe("what wsp does to the lead and the machine", () => {
  it("pushes nothing for the lead and runs nothing on the machine past 10's list", async () => {
    const { rt, boat, daemon, project, lead, token } = await leadOnThisComputer({ picks: [".env"] });
    const before = daemon.frames.length;
    const made = await spawn(rt, project.id, lead);
    expect(daemon.frames.slice(before).map(f => f["op"])).toEqual(["git.status"]);
    const machine = boat.machines[0]!;
    const spawned = [...machine.execLog];
    // The fork road's three, then the folder and the archive, and nothing else; no command carries a byte of a file.
    expect(spawned.filter(forkRoad)).toHaveLength(3);
    expect(spawned.filter(c => !forkRoad(c))).toEqual(["install -d -m 0755 '/root/web'", `tar -xzf '/tmp/wsp-seed-${made.id}.tgz' -C '/root/web' && rm -f '/tmp/wsp-seed-${made.id}.tgz'`]);
    expect(seedFiles(boat)).toEqual([`/tmp/wsp-seed-${made.id}.tgz`]);
    const seed = boat.files.find(f => f.path.startsWith("/tmp/wsp-seed-"))!.bytes.toString("base64");
    expect(spawned.some(c => c.includes(seed.slice(0, 32)))).toBe(false);
    await (await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: "claude", turnToken: token, cwd: made.sub!.folder }, lead)).finished;
    expect(machine.execLog.some(c => /\bgit (clone|push|fetch)\b|\b(npm|pnpm) (install|ci)\b/.test(c))).toBe(false);
  });
});
