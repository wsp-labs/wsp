// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread after its machine was deleted: the agent's one session file kept
// gzipped at each done, or said not kept past 64 MB; a send making a new
// machine from the image head as it stands, landing the picked files and the
// session at the path it was kept from, one copy, and resuming the same session
// under the resume preamble; a box that fails before the turn replaced once;
// and no rewind or fork of a turn the deleted machine ran.
import { rmSync } from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { WebSocketServer, type WebSocket } from "ws";
import { BOX_WORK_DIR, DiskGoneError, NoProviderBackend, type Machine, type MachineBackend } from "@wsp/engine";
import { BOAT_DISK_GONE_LINE, DAEMON_TOKEN_PATH, PIPE_DAEMON_VERSION, diskExpired, diskGoneReleasedLine, SUB_TURN_GONE_LINE, fmtBytes, foldThreads, lostSessionPrompt, subOpening, subResumePreamble, subSessionNotKeptLine, subSessionTooLargeLine, subStayedLine, brokeredAgentRefusal, subHostNoSignInRefusal, subTwiceRefusal, type SubFault } from "@wsp/protocol";
import { DAEMON_TOKEN_SET } from "../src/daemon-token.js";
import { PIPE_IDLE } from "../src/pipe.js";
import { copyKey, type HarnessAdapterFactory, type HarnessStartOptions, type PlaceBackends, type Runtime } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { wiring } from "./place-join.js";
import { SUB_SESSIONS, SUB_SESSION_MAX_BYTES, subSessionLandCommand } from "../src/machines/sub-life.js";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { CLAUDE_SESSION_FILE, CODEX_SESSION_FILE, PUSHED, STORE_TITLE, answering, jumpClock, leadOnThisComputer, sessionPathOf, until, type Answers, type Setup } from "./sub-fixture.js";

const NAME = "Fix the flaky upload test";
const BRANCH = "wsp/fix-the-flaky-upload-test";
const BASE = { branch: "feat/upload", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" };
const MIN = 60_000;

let open: Runtime[] = [];
const closers: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const rt of open) await rt.close().catch(() => undefined);
  open = [];
  for (const close of closers.splice(0)) await close();
});

/** A sub-thread on Boat whose first turn ended, its done read and its session kept, on the agent named. */
async function doneOnce(o: { agent?: "claude" | "codex" | "opencode"; answers?: Partial<Answers>; deleteAfterMs?: number; picks?: string[]; sizes?: Record<string, number> } & Pick<Setup, "broker" | "codex" | "hostGh" | "image" | "storeTitle"> = {}) {
  const agent = o.agent ?? "claude";
  const answers: Answers = { read: PUSHED, session: Buffer.from(`gzipped ${agent} session`), ...o.answers };
  const { clock, jump } = jumpClock();
  const held = await leadOnThisComputer({ clock, picks: o.picks ?? ["CLAUDE.local.md"], ...(o.sizes !== undefined ? { sizes: o.sizes } : {}), ...(o.deleteAfterMs !== undefined ? { spendPerDayUsd: 5 } : {}), ...(o.broker !== undefined ? { broker: o.broker } : {}), ...(o.hostGh !== undefined ? { hostGh: o.hostGh } : {}), ...(o.image !== undefined ? { image: o.image } : {}), ...(o.storeTitle !== undefined ? { storeTitle: o.storeTitle } : {}), ...(o.codex !== undefined ? { codex: o.codex } : {}) });
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

/** A new machine's daemon on a port here: a link is greeted with its hello and every frame on it answered ok, and a
 * pipe is parked. */
async function pipeDaemon(): Promise<{ url: string; parked: WebSocket[] }> {
  const http = createServer();
  const wss = new WebSocketServer({ server: http });
  const parked: WebSocket[] = [];
  wss.on("connection", ws => {
    ws.once("message", data => {
      const frame = JSON.parse(data.toString()) as { id: number; pipe?: boolean };
      ws.send(JSON.stringify({ id: frame.id, ok: true }));
      if (frame.pipe === true) return parked.push(ws);
      ws.send(JSON.stringify({ type: "daemon.hello", root: "/root", version: PIPE_DAEMON_VERSION }));
      ws.on("message", more => ws.send(JSON.stringify({ id: (JSON.parse(more.toString()) as { id: number }).id, ok: true })));
    });
  });
  await new Promise<void>(r => http.listen(0, "127.0.0.1", r));
  closers.push(
    () =>
      new Promise<void>(r => {
        for (const ws of wss.clients) ws.terminate();
        wss.close();
        http.close(() => r());
      }),
  );
  return { url: `http://127.0.0.1:${(http.address() as AddressInfo).port}/`, parked };
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

  it("parks the host's pipes on the new machine of a brokered sub-thread while its turn runs", async () => {
    const held = await released({ broker: true, hostGh: true });
    const d = await pipeDaemon();
    const create = held.boat.create.bind(held.boat);
    held.boat.create = async (spec, waiting) => {
      const m = (await create(spec, waiting)) as Machine;
      m.previewUrl = async () => ({ url: d.url, token: "e", expiresAt: Date.now() + 3_600_000 });
      return m;
    };
    const answer = held.boat.execImpl;
    held.boat.execImpl = (m, cmd, stdin) => (cmd.includes(DAEMON_TOKEN_PATH) ? { exitCode: 0, stdout: `${DAEMON_TOKEN_SET}\n`, stderr: "" } : answer(m, cmd, stdin));
    held.turns.hold = true;
    const turn = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" });
    await until(() => (d.parked.length === PIPE_IDLE ? true : undefined), "the pipes parked on the new machine");
    held.turns.release?.();
    await turn.finished;
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

describe("the title of a sub-thread woken on a new machine", () => {
  // Codex rebuilds its thread index on a new machine from the rollout that landed there and titles the thread with the
  // rollout's first message, wsp's preamble and the task, its name column empty (measured on codex-cli 0.155.1).
  const opening = (starts: readonly HarnessStartOptions[]): string | null => starts.find(s => s.prompt.endsWith("Remember the word lantern."))?.prompt ?? null;
  const titleOf = async (held: { rt: Runtime; threadId: string }): Promise<string | undefined> => foldThreads(await held.rt.sessions.list()).find(t => t.threadId === held.threadId)?.title;
  /** The woken turn run to its end, and the new machine's store read for its title after it. */
  const wake = async (held: Awaited<ReturnType<typeof released>>): Promise<void> => {
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "Run git log --oneline -1.", harness: "codex" })).finished;
    const machine = held.boat.machines.at(-1)!;
    expect(machine.id).not.toBe(held.first.id);
    await until(() => (machine.execLog.some(cmd => cmd.startsWith(STORE_TITLE)) ? true : undefined), "the new machine's title read");
    await new Promise(r => setTimeout(r, 20));
  };

  it("keeps its name when the new machine's store titles it with the preamble it opened with", async () => {
    const held = await released({ agent: "codex", storeTitle: opening });
    expect(held.first.execLog.some(cmd => cmd.startsWith(STORE_TITLE))).toBe(true);
    expect(await titleOf(held)).toBe(NAME);
    await wake(held);
    expect(await titleOf(held)).toBe(NAME);
  });

  it("keeps its name when its session did not land and the new one opens with the thread so far", async () => {
    // Where the host kept no session, the agent starts a new one and hands it the thread so far ahead of the message.
    const held = await released({ agent: "codex", storeTitle: starts => (starts.at(-1)?.resume === undefined ? opening(starts) : lostSessionPrompt(opening(starts) ?? "", starts.at(-1)!.prompt)) });
    await wake(held);
    expect(await titleOf(held)).toBe(NAME);
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

describe("rewind on a sub-thread whose machine napped after its turn", () => {
  it("wakes the machine and cuts the turn, as a send wakes it, where the napped box takes no command", async () => {
    const reverted: string[] = [];
    // Codex cuts its thread by a run on the machine, which Boat refuses while the box is stopped.
    const codex = (stub: HarnessAdapterFactory): HarnessAdapterFactory => hctx => ({
      ...stub(hctx),
      revert: async r => {
        await hctx.machine.exec(`codex app-server revert ${r.session}`);
        reverted.push(r.session);
        return undefined;
      },
    });
    const held = await doneOnce({ agent: "codex", codex });
    const answer = held.boat.execImpl;
    held.boat.execImpl = (m, cmd, stdin) => {
      if (m.paused) throw Object.assign(new Error("Sandbox machine is not running."), { kind: "conflict", status: 409, code: "machine_not_running" });
      return answer(m, cmd, stdin);
    };
    await held.rt.workspaces.wake(held.made.id);
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "and the colour?", harness: "codex" })).finished;
    await until(async () => ((await held.record()).phase === "napping" ? true : undefined), "the nap after the second turn");
    expect(held.first.paused).toBe(true);
    const turns = [...new Set((await held.rt.sessions.history(held.made.id)).filter(e => e.threadId === held.threadId && e.turnId !== undefined).map(e => e.turnId!))];
    expect(turns).toHaveLength(2);
    const resumes = held.first.resumes;

    expect(await held.rt.sessions.rewind(held.threadId, { turnId: turns[0]! })).toMatchObject({ turns: 1 });
    expect(held.first.resumes).toBe(resumes + 1);
    expect(reverted).toHaveLength(1);
    expect(new Set((await held.rt.sessions.history(held.made.id)).filter(e => e.threadId === held.threadId && e.turnId !== undefined).map(e => e.turnId!))).toEqual(new Set([turns[0]]));
  });
});

describe("a sub-thread whose disk Boat deleted", () => {
  it("a wake Boat refuses with the disk gone deletes the box, releases the record in one line on its row and its timeline, and the next send starts a new machine", async () => {
    const held = await doneOnce();
    const notices: string[] = [];
    held.rt.events.on("*", e => {
      if (e.type === "session.notice" && e.threadId === held.threadId) notices.push(e.text);
    });
    expect((await held.record()).phase).toBe("napping");
    held.first.resume = async () => {
      throw new DiskGoneError(held.first.id, BOAT_DISK_GONE_LINE, "The sandbox stayed stopped for 14 days. Boat deleted its data.");
    };
    const line = diskGoneReleasedLine(BOAT_DISK_GONE_LINE);
    expect(line).toBe("Boat deleted this machine's disk once it had been paused for 14 days, so it cannot wake; the next send starts a new machine from the image");
    await expect(held.rt.workspaces.wake(held.made.id)).rejects.toThrow(line);
    const r = await held.record();
    expect(r.phase).toBe("released");
    expect(r.machineId).toBe("");
    expect(r.sub!.lost).toBe(line);
    expect(r.diskGone).toBe(line);
    expect(held.first.killed).toBe(true);
    expect(notices).toEqual([line]);
    expect((await held.rt.sessions.list()).filter(s => s.threadId === held.threadId).at(-1)!.diskGone).toBe(line);
    const made = held.boat.machines.length;
    await (await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "what was the word?", harness: "claude" })).finished;
    expect(held.boat.machines).toHaveLength(made + 1);
    const machine = held.boat.machines.at(-1)!;
    expect(machine.id).not.toBe(held.first.id);
    const after = await until(async () => ((await held.record()).machineId === machine.id ? held.record() : undefined), "the record on the new machine");
    expect(after.sub!.lost).toBeUndefined();
    expect(after.diskGone).toBeUndefined();
  });
});

describe("the keep of a napping machine's disk", () => {
  const DAY = 24 * 60 * MIN;

  it("a thread whose machine napped carries the day its cloud's keep of the disk runs out, reads expired from that day, and carries none once it wakes", async () => {
    const held = await doneOnce();
    held.boat.lifecycle.diskKeptMs = 14 * DAY;
    const r = await held.record();
    expect(r.phase).toBe("napping");
    expect(r.nappedAt).toBeTypeOf("number");
    const until14 = r.nappedAt! + 14 * DAY;
    const rows = async () => (await held.rt.sessions.list()).filter(s => s.threadId === held.threadId);
    expect((await rows()).at(-1)!.diskKeptUntil).toBe(until14);
    const thread = foldThreads(await rows())[0]!;
    expect(thread.diskKeptUntil).toBe(until14);
    expect(diskExpired(thread, until14 - 1)).toBe(false);
    expect(diskExpired(thread, until14)).toBe(true);
    await held.rt.workspaces.wake(held.made.id);
    expect((await rows()).at(-1)!.diskKeptUntil).toBeUndefined();
  });

  it("a host started over a box its cloud stopped before counts the keep from the cloud's own stop", async () => {
    const held = await doneOnce();
    held.boat.lifecycle.diskKeptMs = 14 * DAY;
    held.first.pausedAt = "2026-09-27T04:50:29.157Z";
    await held.rt.close();
    open = open.filter(rt => rt !== held.rt);
    const again = held.restart();
    open.push(again);
    const stop = Date.parse("2026-09-27T04:50:29.157Z");
    const row = await until(async () => (await again.sessions.list()).find(s => s.threadId === held.threadId && s.diskKeptUntil !== undefined), "the row with its keep");
    expect(row.diskKeptUntil).toBe(stop + 14 * DAY);
    expect((await held.record()).nappedAt).toBe(stop);
  });
});

describe("a box a released sub-thread's record does not name", () => {
  it("is named by the sweep with the time it goes and deleted a day on, the record left released", async () => {
    const held = await released();
    const left = await held.boat.create({ kind: "sandbox", labels: { ...held.first.spec.labels!, createdAt: new Date(Date.now() - 2 * MIN).toISOString() } });
    held.boat.machines.find(m => m.id === left.id)!.paused = true;

    expect((await held.rt.reap()).failed?.find(f => f.id === left.id)?.message).toMatch(new RegExp(`^not recorded: ${held.made.id} holds no machine, so nothing here reaches this paused machine of yours; deleted at \\S+, a day after this host first saw it$`));
    held.jump(24 * 60 * MIN);
    expect((await held.rt.reap()).reaped).toContainEqual(expect.objectContaining({ id: left.id, reason: "unrecorded" }));
    expect(held.boat.machines.find(m => m.id === left.id)!.killed).toBe(true);
    expect((await held.record()).phase).toBe("released");
  });
});

describe("a box a send into a released sub-thread is making", () => {
  for (const forksOnNone of [false, true]) {
    it(`is not killed by a sweep before its create returns${forksOnNone ? ", on a host that holds Boat as a place" : ""}`, async () => {
      const { clock, jump } = jumpClock();
      let boat: MachineBackend | undefined;
      const none = new NoProviderBackend();
      const host = forksOnNone ? { backend: none, places: { wired: "none", backend: (p: string) => (p === "box" ? boat : p === "none" ? none : undefined), list: () => ["box"] } satisfies PlaceBackends, placeLinks: wiring(newPlaceKeyPair()) } : {};
      const held = await leadOnThisComputer({ clock, picks: ["CLAUDE.local.md"], host } as Parameters<typeof leadOnThisComputer>[0]);
      boat = held.boat;
      open.push(held.rt);
      answering(held.boat, { read: PUSHED, session: Buffer.from("gzipped claude session") });
      const made = await held.rt.workspaces.create({ project: held.project.id, name: "probe", sub: { on: "boat", harness: "claude" } }, held.lead);
      const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Remember the word lantern."), harness: "claude", notify: ["me"], turnToken: held.token, title: "probe", cwd: made.sub!.folder }, held.lead);
      await child.finished;
      const phase = async () => ((await held.store.get("workspaces", made.id)) as WorkspaceRecord).phase;
      await until(async () => (["napping", "released"].includes(await phase()) ? true : undefined), "the nap");
      jump(31 * 60_000);
      await until(async () => ((await phase()) === "released" ? true : undefined), "the release");
      const inner = held.boat.create.bind(held.boat);
      let release!: () => void;
      const gate = new Promise<void>(r => (release = r));
      let box: { id: string; killed: boolean; spec: { labels: Record<string, string> } } | undefined;
      held.boat.create = async (spec, waiting) => {
        const m = await inner(spec, waiting);
        // The real create tells the box's first state, with its id, as the POST answers; answering() drops waiting.
        waiting?.("provisioning", { id: m.id, state: "provisioning" });
        box = m as unknown as typeof box;
        box!.spec.labels["createdAt"] = new Date(Date.now() - 2 * 60_000).toISOString();
        await gate;
        return m;
      };
      const send = held.rt.sessions.start(made.id, { thread: child.view().threadId!, prompt: "what was the word?", harness: "claude" }).catch((e: unknown) => e);
      await until(() => (box !== undefined ? true : undefined), "the create asked of Boat");

      expect((await held.rt.reap()).reaped).toEqual([]);
      expect(box!.killed).toBe(false);
      release();
      await send;
    });
  }
});
