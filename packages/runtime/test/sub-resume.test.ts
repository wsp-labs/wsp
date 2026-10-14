// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread after its machine was deleted: the agent's one session file kept
// gzipped at each done, or said not kept past 64 MB; a send making a new
// machine from the image head as it stands, landing the picked files and the
// session at the path it was kept from, one copy, and resuming the same session
// under the resume preamble; a box that fails before the turn replaced once;
// and no rewind or fork of a turn the deleted machine ran.
import { rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { BOX_WORK_DIR } from "@wsp/engine";
import { SUB_TURN_GONE_LINE, fmtBytes, subOpening, subResumePreamble, subSessionNotKeptLine, subSessionTooLargeLine, subStayedLine, brokeredAgentRefusal, subHostNoSignInRefusal, subTwiceRefusal, type SubFault } from "@wsp/protocol";
import { copyKey, type Runtime } from "../src/runtime.js";
import { SUB_SESSIONS, SUB_SESSION_MAX_BYTES, subSessionLandCommand } from "../src/machines/sub-life.js";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { CLAUDE_SESSION_FILE, CODEX_SESSION_FILE, PUSHED, answering, jumpClock, leadOnThisComputer, sessionPathOf, until, type Answers, type Setup } from "./sub-fixture.js";

const NAME = "Fix the flaky upload test";
const BRANCH = "wsp/fix-the-flaky-upload-test";
const BASE = { branch: "feat/upload", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" };
const MIN = 60_000;

let open: Runtime[] = [];
afterEach(async () => {
  for (const rt of open) await rt.close().catch(() => undefined);
  open = [];
});

/** A sub-thread on Boat whose first turn ended, its done read and its session kept, on the agent named. */
async function doneOnce(o: { agent?: "claude" | "codex" | "opencode"; answers?: Partial<Answers>; deleteAfterMs?: number; picks?: string[]; sizes?: Record<string, number> } & Pick<Setup, "broker" | "hostGh" | "image"> = {}) {
  const agent = o.agent ?? "claude";
  const answers: Answers = { read: PUSHED, session: Buffer.from(`gzipped ${agent} session`), ...o.answers };
  const { clock, jump } = jumpClock();
  const held = await leadOnThisComputer({ clock, picks: o.picks ?? ["CLAUDE.local.md"], ...(o.sizes !== undefined ? { sizes: o.sizes } : {}), ...(o.deleteAfterMs !== undefined ? { spendPerDayUsd: 5 } : {}), ...(o.broker !== undefined ? { broker: o.broker } : {}), ...(o.hostGh !== undefined ? { hostGh: o.hostGh } : {}), ...(o.image !== undefined ? { image: o.image } : {}) });
  open.push(held.rt);
  if (o.deleteAfterMs !== undefined) await held.rt.places!.set("box", { deleteAfterMs: o.deleteAfterMs });
  answering(held.boat, answers);
  const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat", harness: agent } }, held.lead);
  const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Remember the word lantern."), harness: agent, notify: ["me"], turnToken: held.token, title: NAME, cwd: made.sub!.folder }, held.lead);
  const first = held.boat.machines.at(-1)!;
  const threadId = child.view().threadId!;
  await child.finished;
  const record = (): Promise<WorkspaceRecord> => held.store.get("workspaces", made.id) as Promise<WorkspaceRecord>;
  await until(async () => (["napping", "released"].includes((await record()).phase) ? true : undefined), "the nap at the done");
  return { ...held, agent, answers, made, first, threadId, record, clock, jump };
}

/** The same sub-thread once the delay passed and its box was deleted at Boat. */
async function released(o: Parameters<typeof doneOnce>[0] = {}) {
  const held = await doneOnce(o);
  held.jump(31 * MIN);
  await until(async () => ((await held.record()).phase === "released" ? true : undefined), "the release");
  return held;
}

/** The image on Boat moved on to v2 since the sub-thread started. */
async function sealV2(held: { store: Awaited<ReturnType<typeof leadOnThisComputer>>["store"] }): Promise<void> {
  const v = (n: number) => ({ version: n, snapshotId: `wsp-host-default-v${n}`, baseTemplate: "base", setupSha: "abc", createdAt: "2026-10-07T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 }, logins: [{ name: "GitHub CLI login", state: "copied" }] });
  await held.store.put("goldens", copyKey("box", "default"), { head: 2, versions: [v(1), v(2)] });
}

const sessionFiles = (held: { boat: { files: { machine: string; path: string; bytes: Buffer }[] } }, machine: string) => held.boat.files.filter(f => f.machine === machine && f.path.startsWith("/tmp/wsp-session-"));

describe("the session kept at each done", () => {
  it("keeps the agent's one file gzipped on the host by the path it stood at, and the next done's in its place", async () => {
    const held = await doneOnce();
    const r = await held.record();
    expect(r.sub!.session).toMatchObject({ agent: "claude", id: "s2", path: "/root/.claude-cfg/projects/-root-web/s2.jsonl", bytes: (held.answers.session as Buffer).length, kept: expect.any(Number) });
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toEqual(held.answers.session);
    // A second turn on the same machine, and its done keeps the session as that turn left it.
    await held.rt.workspaces.wake(held.made.id);
    held.answers.session = Buffer.from("the session after turn two");
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "and one more", harness: "claude" })).finished;
    await until(async () => ((await held.store.getBlob(SUB_SESSIONS, held.threadId))?.toString() === "the session after turn two" ? true : undefined), "the second keep");
    const row = (await held.rt.sessions.list()).find(s => s.threadId === held.threadId)!;
    expect(row.sub?.sessionNotKept).toBeUndefined();
  });

  it("keeps none past 64 MB packed, drops the copy it held, and the row says a send goes on from the transcript", async () => {
    const held = await doneOnce();
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toBeDefined();
    await held.rt.workspaces.wake(held.made.id);
    const size = SUB_SESSION_MAX_BYTES + 1;
    held.answers.session = size;
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "read the whole log", harness: "claude" })).finished;
    await until(async () => ((await held.record()).sub!.session?.kept === null ? true : undefined), "the keep refused");
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toBeUndefined();
    const row = (await held.rt.sessions.list()).find(s => s.threadId === held.threadId)!;
    expect(row.sub?.sessionNotKept).toBe(subSessionTooLargeLine(fmtBytes(size)));
    expect(row.sub?.sessionNotKept).toBe(`the agent's session was too large to keep (${fmtBytes(size)}); a send continues from the transcript`);
  });
});

describe("a keep that fails after a good one", () => {
  it("keeps none and drops the copy from the done before, so the row says a send goes on from the transcript", async () => {
    const held = await doneOnce();
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toBeDefined();
    await held.rt.workspaces.wake(held.made.id);
    // The pack answers a size, and the read of it fails.
    held.answers.session = 1_234;
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "and one more", harness: "claude" })).finished;
    const r = await until(async () => ((await held.record()).sub!.session?.kept === null ? held.record() : undefined), "the failed keep");
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toBeUndefined();
    const row = (await held.rt.sessions.list()).find(s => s.threadId === held.threadId)!;
    expect(row.sub?.sessionNotKept).toBe(subSessionNotKeptLine(r.sub!.session!.why!));
    expect(r.sub!.session!.why).toContain("nothing packed at /tmp/wsp-session-");
  });
});

describe("a send into a released sub-thread", () => {
  it("makes a send that arrives while the new machine is made wait for it, so the turn with the preamble runs first and none runs on the deleted box", async () => {
    const held = await released();
    let open!: () => void;
    const gate = new Promise<void>(r => (open = r));
    const create = held.boat.create.bind(held.boat);
    let asked = false;
    held.boat.create = async spec => {
      asked = true;
      await gate;
      return create(spec);
    };
    const launches = held.starts.length;
    const a = held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" });
    await until(() => (asked ? true : undefined), "the create asked of Boat");
    const b = held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "and the second word?", harness: "claude" });
    await new Promise(r => setTimeout(r, 30));
    // Nothing has launched while the machine is being made.
    expect(held.starts).toHaveLength(launches);
    // The thread's row reads its machine starting meanwhile, the resume's own start under way on it.
    const row = (await held.rt.sessions.list()).filter(s => s.threadId === held.threadId).at(-1)!;
    expect(row.sub).toMatchObject({ state: "starting", spawn: { machine: { states: [] } } });
    expect(row.sub!.spawn!.branch).toBeUndefined();
    open();
    await Promise.all([(await a).finished, (await b).finished]);
    expect((await held.record()).sub!.spawn).toMatchObject({ readyAt: expect.any(Number), machine: { ms: expect.any(Number) } });
    // A start that stands drops the asked text: the turn's own row and transcript hold it from here.
    expect((await held.record()).sub!.spawn!.asked).toBeUndefined();
    const ran = held.starts.slice(launches).map(x => x.o.prompt);
    expect(ran).toHaveLength(2);
    expect(ran[0]!.startsWith("This thread's machine was deleted after its last turn")).toBe(true);
    expect(ran[0]!.endsWith("what was the word?")).toBe(true);
    expect(ran[1]).toBe("and the second word?");
    expect(held.boat.creates.filter(c => c.fromSnapshot !== undefined).length).toBe(2);
  });

  it("runs a done for a send refused after its new machine stands, so the delete is armed and the branch pushed before stands", async () => {
    const held = await released();
    held.answers.read = "norepo\n";
    const png = { mediaType: "image/png", bytes: Buffer.alloc(32, 7).toString("base64"), name: "shot.png" };
    const refused = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "look at this", harness: "claude", attachments: [png] }).then(
      () => undefined,
      (e: unknown) => e as Error,
    );
    expect(refused).toBeInstanceOf(Error);
    const machine = held.boat.machines.at(-1)!;
    expect(machine.id).not.toBe(held.first.id);
    const r = await until(async () => ((await held.record()).phase === "napping" ? held.record() : undefined), "the nap after the refused send");
    expect(r.machineId).toBe(machine.id);
    expect(r.sub!.deleteAt).toBeGreaterThan(held.clock.now());
    expect(r.sub!.result).toMatchObject({ pushedSha: "9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e", commits: 3 });
    expect(r.sub!.result!.norepo).toBeUndefined();
  });

  it("keeps the branch pushed before when the delete reads a woken new machine that holds no repository", async () => {
    const held = await released();
    held.answers.read = "norepo\n";
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" })).finished;
    await until(async () => ((await held.record()).phase === "napping" ? true : undefined), "the nap after the resumed turn");
    await held.rt.workspaces.wake(held.made.id);
    held.jump(31 * MIN);
    const r = await until(async () => ((await held.record()).phase === "released" ? held.record() : undefined), "the second release");
    expect(r.sub!.result).toMatchObject({ pushedSha: "9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e", commits: 3 });
    expect(r.sub!.result!.norepo).toBeUndefined();
  });

  it("makes a new machine from the image head, lands the picked files and Claude's session in its -root-web folder, one copy, and resumes the same session under the preamble", async () => {
    const held = await released();
    await sealV2(held);
    const creates = held.boat.creates.length;
    const turn = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" });
    await turn.finished;
    // One more start at Boat, forked from the head as it stands now, at the record's size.
    expect(held.boat.creates).toHaveLength(creates + 1);
    expect(held.boat.creates.at(-1)).toMatchObject({ fromSnapshot: "wsp-host-default-v2", cpu: 2, memMb: 4096 });
    const machine = held.boat.machines.at(-1)!;
    expect(machine.id).not.toBe(held.first.id);
    expect(held.first.killed).toBe(true);
    // The picked files, and the kept session put once, then landed where it stood with any other copy taken away.
    expect(held.boat.files.filter(f => f.machine === machine.id && !f.path.startsWith("/tmp/wsp-vault-in-")).map(f => f.path)).toEqual([`/tmp/wsp-seed-${held.made.id}.tgz`, `/tmp/wsp-session-${held.threadId}.gz`]);
    expect(sessionFiles(held, machine.id)[0]!.bytes).toEqual(held.answers.session);
    const at = "/root/.claude-cfg/projects/-root-web/s2.jsonl";
    const land = subSessionLandCommand(CLAUDE_SESSION_FILE("s2"), at, `/tmp/wsp-session-${held.threadId}.gz`);
    expect(machine.execLog).toContain(land);
    expect(land).toContain(`find '/root/.claude-cfg/projects' -maxdepth 2 -type f -name '*s2.jsonl' ! -path '${at}' -delete`);
    expect(land).toContain(`gunzip -c '/tmp/wsp-session-${held.threadId}.gz' > '${at}'`);
    // The session landed before the agent's launch, which resumes it by the same id under the resume preamble.
    const launch = held.starts.at(-1)!.o;
    expect(launch.resume).toBe("s2");
    expect(launch.seed).toBeDefined();
    const preamble = subResumePreamble({ on: "Boat", size: { name: "small", cpu: 2, memMb: 4096 }, folder: "/root/web", sent: ["CLAUDE.local.md"], setupGit: "gh auth setup-git", repo: "https://github.com/acme/web.git", base: BASE, branch: BRANCH, pushed: true, deleteAfterMinutes: 30 });
    expect(launch.prompt).toBe(subOpening(preamble, "what was the word?"));
    expect(preamble).toBe(
      [
        "This thread's machine was deleted after its last turn, and the thread goes on here, on a new machine on Boat (small, 2 vCPU, 4 GB). Your conversation came with it; the code did not. The folder you are in, /root/web, holds only the git-ignored files the person picked: CLAUDE.local.md. Get your branch back first, in this folder:",
        "",
        "gh auth setup-git",
        "git init -q",
        "git remote add origin https://github.com/acme/web.git",
        `git fetch -q --filter=blob:none origin ${BRANCH}`,
        `git checkout -q -b ${BRANCH} FETCH_HEAD`,
        "",
        `Push to ${BRANCH} as before. This machine is deleted 30 minutes after your last turn ends.`,
        "",
        "The message:",
      ].join("\n"),
    );
    const landedAt = machine.execLog.indexOf(land);
    expect(landedAt).toBeGreaterThan(machine.execLog.findIndex(c => c.startsWith("tar -xzf")));
    // The record runs on the new machine, from the new head, and its done naps it with the delete armed again.
    const r = await until(async () => ((await held.record()).phase === "napping" ? held.record() : undefined), "the nap after the resumed turn");
    expect(r).toMatchObject({ machineId: machine.id, golden: "wsp-host-default-v2" });
    expect(r.sub!.deleteAt).toBeGreaterThan(held.clock.now());
  });

  it("cuts the first machine off at the broker before Boat deletes it, and lifts it before the new machine a send makes under the same record", async () => {
    const held = await doneOnce({ broker: false });
    const kill = held.first.kill.bind(held.first);
    const offAtKill: boolean[] = [];
    held.first.kill = async () => {
      offAtKill.push(held.brokerSwitch.off.has(held.made.id));
      await kill();
    };
    held.jump(31 * MIN);
    await until(async () => ((await held.record()).phase === "released" ? true : undefined), "the release");
    expect(offAtKill).toEqual([true]);
    expect(held.brokerSwitch.off.has(held.made.id)).toBe(true);
    const create = held.boat.create.bind(held.boat);
    const offAtCreate: boolean[] = [];
    held.boat.create = async spec => {
      offAtCreate.push(held.brokerSwitch.off.has(held.made.id));
      return create(spec);
    };
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" })).finished;
    expect(offAtCreate).toEqual([false]);
  });

  it("names a picked file of values that stayed on this computer in the notice of the send that makes the new machine", async () => {
    const held = await released({ picks: [".env", "CLAUDE.local.md"], sizes: { ".env": 120, "CLAUDE.local.md": 1100 } });
    const turn = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" });
    await turn.finished;
    expect(turn.notice?.split("\n")).toContain(subStayedLine([".env"]));
  });

  it("names a picked file of values that stayed on this computer when a first start that failed is tried again", async () => {
    const { rt, boat, project, lead, token } = await leadOnThisComputer({ picks: [".env", "CLAUDE.local.md"], sizes: { ".env": 120, "CLAUDE.local.md": 1100 } });
    open.push(rt);
    boat.faults = [{ kind: "unusable", replayed: true }, { kind: "unusable" }];
    const made = await rt.workspaces.create({ project: project.id, name: NAME, sub: { on: "boat" } }, lead);
    const opening = subOpening(made.sub!.preamble, "Run the suite.");
    await rt.sessions.start(made.id, { prompt: opening, harness: made.sub!.harness, turnToken: token, cwd: made.sub!.folder }, lead).catch(() => undefined);
    const threadId = (await rt.sessions.list()).find(r => r.workspaceId === made.id)!.threadId!;
    const tried = await rt.sessions.start(made.id, { thread: threadId, prompt: opening, harness: "claude", cwd: made.sub!.folder }, lead);
    await tried.finished;
    expect(tried.notice?.split("\n")).toContain(subStayedLine([".env"]));
  });

  it("lands Codex's rollout at its dated path under the box's home, one copy, and resumes the same thread", async () => {
    const held = await released({ agent: "codex" });
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "codex" })).finished;
    const machine = held.boat.machines.at(-1)!;
    const at = sessionPathOf("/root/.codex/sessions", "-s2.jsonl");
    expect(at).toBe("/root/.codex/sessions/2026/10/12/rollout-2026-10-12T06-00-00-s2.jsonl");
    expect((await held.record()).sub!.session?.path).toBe(at);
    expect(sessionFiles(held, machine.id).map(f => f.bytes)).toEqual([held.answers.session]);
    expect(machine.execLog).toContain(subSessionLandCommand(CODEX_SESSION_FILE("s2"), at, `/tmp/wsp-session-${held.threadId}.gz`));
    expect(held.starts.at(-1)!.o).toMatchObject({ resume: "s2" });
  });

  it("replaces a box that cannot run commands before the turn once, and lands the session on the second alone", async () => {
    const held = await released();
    held.boat.faults.push({ kind: "unusable" });
    const creates = held.boat.creates.length;
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" })).finished;
    expect(held.boat.creates).toHaveLength(creates + 2);
    const [broken, good] = held.boat.machines.slice(-2);
    expect(broken!.killed).toBe(true);
    expect(sessionFiles(held, broken!.id)).toEqual([]);
    expect(sessionFiles(held, good!.id)).toHaveLength(1);
    expect(held.starts.at(-1)!.o.resume).toBe("s2");
    expect((await held.record()).machineId).toBe(good!.id);
  });

  it("puts the failed start's row on the bus before it answers the refusal, however long the row takes to place", async () => {
    const held = await released({ deleteAfterMs: 30 * MIN });
    held.boat.faults.push({ kind: "unusable" }, { kind: "unusable" });
    // Placing a row reads the computers; a slow read must not let the refusal overtake the row.
    const door = held.rt.places!;
    const rows = door.rows.bind(door);
    door.rows = async () => (await new Promise(r => setTimeout(r, 50)), rows());
    const order: string[] = [];
    held.rt.events.on("*", e => void (e.type === "session.row" && e.row?.sub?.spawn?.failed !== undefined && order.push("row")));
    await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" }).then(
      () => order.push("started"),
      () => order.push("refused"),
    );
    expect(order[0]).toBe("row");
    expect(order).toContain("refused");
  });

  it("fails after a second broken box with both named, and the record stays released for the next send", async () => {
    const held = await released();
    held.boat.faults.push({ kind: "unusable" }, { kind: "unusable" });
    const launches = held.starts.length;
    const e = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" }).then(
      () => undefined,
      (x: unknown) => x as Error,
    );
    const [a, b] = held.boat.machines.slice(-2);
    const fault = (box: string): SubFault => ({ kind: "unusable", box, why: `${BOX_WORK_DIR} is missing` });
    expect(e?.message).toBe(subTwiceRefusal(fault(a!.id), fault(b!.id), NAME, "Boat").message);
    expect(held.starts).toHaveLength(launches);
    expect(await held.record()).toMatchObject({ phase: "released", machineId: "" });
    // The step it failed on stays on the row with the two halves of why, for the line under the message to say.
    // And the message the send asked, which no turn wrote down, for Try again after a reload too.
    expect((await held.record()).sub!.spawn!.asked).toBe("what was the word?");
    const failed = (await held.record()).sub!.spawn!.failed!;
    expect(failed).toEqual({ step: "machine", said: e!.message.slice(0, -" Start it again in a few minutes.".length), fix: "Start it again in a few minutes." });
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" })).finished;
    expect(held.starts.at(-1)!.o.resume).toBe("s2");
  });

  it("asks gh on this computer again before a new machine while the broker is ready, and refuses with no machine made where it holds no login", async () => {
    const held = await released({ broker: true, hostGh: true });
    const machines = held.boat.machines.length;
    rmSync(join(dirname(held.repo), "gh-signed-in"));
    const e = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" }).then(
      () => undefined,
      (x: unknown) => x as Error,
    );
    expect(e?.message).toBe(subHostNoSignInRefusal("Boat", "acme/web").message);
    expect(held.boat.machines).toHaveLength(machines);
    expect(await held.record()).toMatchObject({ phase: "released", machineId: "" });
  });

  it("refuses a sub-thread on an agent the broker adds no key for before a new machine once the broker is ready", async () => {
    // Started on OpenCode while the broker was not up, then the broker came up.
    const held = await released({ agent: "opencode", broker: false, hostGh: true, image: { agents: ["claude", "codex", "opencode"] } });
    held.brokerSwitch.ready = true;
    const machines = held.boat.machines.length;
    const e = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "opencode" }).then(
      () => undefined,
      (x: unknown) => x as Error,
    );
    expect(e?.message).toBe(brokeredAgentRefusal("OpenCode", ["Claude Code", "Codex"]).message);
    expect(held.boat.machines).toHaveLength(machines);
  });

  it("with no session kept lands none and resumes by the id with the transcript at hand, which the agent falls back to", async () => {
    const held = await released({ answers: { session: SUB_SESSION_MAX_BYTES + 1 } });
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" })).finished;
    const machine = held.boat.machines.at(-1)!;
    expect(sessionFiles(held, machine.id)).toEqual([]);
    expect(machine.execLog.some(c => c.includes("gunzip -c"))).toBe(false);
    const launch = held.starts.at(-1)!.o;
    expect(launch.resume).toBe("s2");
    expect(await launch.seed!()).toContain("Remember the word lantern.");
  });

  it("starts a sub-thread that pushed nothing again from the lead's commit it started on", async () => {
    const held = await released({ answers: { read: "norepo\n" } });
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "go on", harness: "claude" })).finished;
    const prompt = held.starts.at(-1)!.o.prompt;
    expect(prompt).toContain(`git fetch -q --filter=blob:none origin ${BASE.branch}\ngit checkout -q -b ${BRANCH} ${BASE.sha}\n`);
    expect(prompt).toContain(`Work on ${BRANCH}, then commit and run git push -u origin ${BRANCH} before your turn ends.`);
  });
});

describe("rewind and fork on a sub-thread whose machine was deleted", () => {
  it("refuses either on a turn the deleted machine ran in L11's words, and leaves a turn the new machine ran to the usual rules", async () => {
    // Deleted at once, so the turns' times and the release's are read off one clock, as on a host.
    const held = await doneOnce({ deleteAfterMs: 0 });
    await until(async () => ((await held.record()).phase === "released" ? true : undefined), "the release");
    await held.rt.places!.set("box", { deleteAfterMs: 30 * MIN });
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" })).finished;
    await until(async () => ((await held.record()).phase === "napping" ? true : undefined), "the nap after the resumed turn");
    const turns = [...new Set((await held.rt.sessions.history(held.made.id)).filter(e => e.threadId === held.threadId && e.turnId !== undefined).map(e => e.turnId!))];
    expect(turns).toHaveLength(2);
    const said = (p: Promise<unknown>) =>
      p.then(
        () => undefined,
        (e: unknown) => e as Error & { kind?: string },
      );
    const rewind = await said(held.rt.sessions.rewind(held.threadId, { turnId: turns[0]! }));
    expect(rewind?.message).toBe(SUB_TURN_GONE_LINE);
    expect(rewind?.message).toBe("that turn ran on a machine since deleted, so its files cannot be rewound; send to the thread to go on from where it stands.");
    expect(rewind?.kind).toBe("conflict");
    const fork = await said(held.rt.sessions.fork({ prompt: "x", fork: { threadId: held.threadId, turnId: turns[0]! } }));
    expect(fork?.message).toBe(SUB_TURN_GONE_LINE);
    const later = await said(held.rt.sessions.fork({ prompt: "x", fork: { threadId: held.threadId, turnId: turns[1]! } }));
    expect(later?.message).not.toBe(SUB_TURN_GONE_LINE);
  });
});
