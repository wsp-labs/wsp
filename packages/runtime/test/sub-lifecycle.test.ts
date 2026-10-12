// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's machine from its done to its delete: the result read and the
// session kept at each done, the delete armed or the machine held, the nap at
// once, the delete at deleteAt with the record released and everything but the
// box kept, the timer across a restart, a box Boat lost, a sub-thread deleted,
// and Boat removed. Boat is the stub backend of the spawn's own fixture; time
// moves by a clock the test jumps, every other timer running for real.
import { afterEach, describe, expect, it } from "vitest";
import { exitClassOf, type SessionRowEvent, subCloudGoneRefusal, subHeldDeleteRefusal, subOpening, subReleasedRefusal, type Caller, type SubResult } from "@wsp/protocol";
import type { Clock } from "../src/clock.js";
import { SUB_SESSIONS, subResultCommand } from "../src/machines/sub-life.js";
import type { Runtime } from "../src/runtime.js";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { PUSHED, UNPUSHED, answering, jumpClock, leadOnThisComputer, until, type Answers } from "./sub-fixture.js";
import type { StubMachine } from "./stub-backend.js";

const NAME = "Fix the flaky upload test";
const BRANCH = "wsp/fix-the-flaky-upload-test";
const BASE = "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a";
const PUSHED_SHA = "9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e";
const MIN = 60_000;
type Says = NonNullable<Awaited<ReturnType<typeof leadOnThisComputer>>["turns"]["says"]>;
/** The agent's push of the branch and what the remote answered, as Claude Code reports a command that failed. */
const pushRefused = (output: string, exitCode = 128): Says => [
  { kind: "tool_use", toolName: "Bash", toolUseId: "push-1", text: JSON.stringify({ command: `git push -u origin ${BRANCH}` }) },
  { kind: "tool_result", toolName: "Bash", toolUseId: "push-1", isError: true, exitCode, text: output },
];
/** git 2.43's answer to a push with a token that cannot write, as measured against github.com on 2026-10-12. */
const DENIED = "remote: Permission to acme/web.git denied to wsp-bot.\nfatal: unable to access 'https://github.com/acme/web.git/': The requested URL returned error: 403";

let open: Runtime[] = [];
afterEach(async () => {
  for (const rt of open) await rt.close().catch(() => undefined);
  open = [];
});

/** A lead on this computer, its sub-thread on Boat started and its first turn ended, the done steps run. */
async function doneOnce(o: { answers?: Partial<Answers>; clock?: Clock; deleteAfterMs?: number; says?: Says } = {}) {
  const answers: Answers = { read: PUSHED, session: Buffer.from("gzipped session bytes"), ...o.answers };
  const held = await leadOnThisComputer({ ...(o.clock !== undefined ? { clock: o.clock } : {}), ...(o.deleteAfterMs !== undefined ? { spendPerDayUsd: 5 } : {}) });
  open.push(held.rt);
  if (o.says !== undefined) held.turns.says = o.says;
  const pushed: SessionRowEvent[] = [];
  held.rt.events.on("*", e => void (e.type === "session.row" && pushed.push(e)));
  if (o.deleteAfterMs !== undefined) await held.rt.places!.set("box", { deleteAfterMs: o.deleteAfterMs });
  answering(held.boat, answers);
  const backstops: { machine: string; until: number; exact: boolean }[] = [];
  held.boat.lifecycle.backstop = async (m, at, x) => void backstops.push({ machine: m.id, until: at, exact: x?.exact === true });
  const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
  const machine = held.boat.machines.at(-1)!;
  const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", notify: ["me"], turnToken: held.token, title: NAME, cwd: made.sub!.folder }, held.lead);
  const threadId = child.view().threadId!;
  await child.finished;
  const record = (): Promise<WorkspaceRecord> => held.store.get("workspaces", made.id) as Promise<WorkspaceRecord>;
  await until(async () => (await record()).sub?.result, "the done read");
  return { ...held, answers, made, machine, threadId, record, backstops, pushed };
}

/** The record once its nap has landed. */
const settled = async (held: { record: () => Promise<WorkspaceRecord> }): Promise<WorkspaceRecord> => until(async () => ((await held.record()).phase === "napping" ? held.record() : undefined), "the nap");

describe("a sub-thread's done", () => {
  it("reads the result off local refs, keeps the session on the host, arms the delete at the delay and naps the machine", async () => {
    const held = await doneOnce();
    const r = await settled(held);
    const { result, deleteAt, held: kept, session } = r.sub!;
    expect(result).toEqual({ branch: BRANCH, pushedSha: PUSHED_SHA, commits: 3, unpushed: 0, changed: 0, onBranch: BRANCH, readAt: expect.any(String) } satisfies SubResult);
    expect(held.machine.execLog).toContain(subResultCommand("/root/web", BRANCH, BASE));
    expect(kept).toBeUndefined();
    expect(deleteAt! - Date.parse(result!.readAt)).toBeGreaterThanOrEqual(30 * MIN - 1_000);
    expect(deleteAt! - Date.parse(result!.readAt)).toBeLessThanOrEqual(30 * MIN + 5_000);
    // The one session file, packed under /tmp where Boat's file read reaches, read off and kept by thread.
    const pack = held.machine.execLog.find(c => c.includes("gzip -c"))!;
    expect(pack).toContain("find '/root/.claude-cfg/projects' -maxdepth 2 -type f -name '*s2.jsonl'");
    expect(pack).toContain(`gzip -c "$f" > '/tmp/wsp-session-${held.threadId}.gz'`);
    expect(held.machine.execLog).toContain(`rm -f '/tmp/wsp-session-${held.threadId}.gz'`);
    expect(session).toMatchObject({ agent: "claude", id: "s2", bytes: (held.answers.session as Buffer).length, kept: expect.any(Number) });
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toEqual(held.answers.session);
    // L4: napped at once, Boat asked to stop it.
    expect(r.phase).toBe("napping");
    expect(held.machine.paused).toBe(true);
    // The row says so.
    const row = (await held.rt.sessions.list()).find(s => s.threadId === held.threadId)!;
    expect(row.sub).toMatchObject({ state: "napping", on: "boat", machine: NAME, branch: BRANCH, deleteAt, result });
  });

  it("arms the delete at the delay the person set on Boat, read at the done", async () => {
    const held = await doneOnce({ deleteAfterMs: 2 * MIN });
    const r = await settled(held);
    const armed = r.sub!.deleteAt! - Date.parse(r.sub!.result!.readAt);
    expect(armed).toBeGreaterThanOrEqual(2 * MIN - 1_000);
    expect(armed).toBeLessThanOrEqual(2 * MIN + 5_000);
  });

  it("holds the machine over work not pushed, arms no delete, and naps it all the same", async () => {
    const held = await doneOnce({ answers: { read: UNPUSHED } });
    const r = await settled(held);
    expect(r.sub!.held).toMatchObject({ files: 3, commits: 2 });
    expect(r.sub!.deleteAt).toBeUndefined();
    expect(r.sub!.result).toMatchObject({ pushedSha: null, unpushed: 2, changed: 3 });
    expect(r.phase).toBe("napping");
    expect(held.machine.paused).toBe(true);
  });

  it("tells every window the row once the done read holds the machine, so its tile reads Not pushed with no reload", async () => {
    const held = await doneOnce({ answers: { read: UNPUSHED } });
    await settled(held);
    await until(async () => held.pushed.find(e => e.threadId === held.threadId && e.row?.sub?.held !== undefined), "a pushed row carrying the hold");
  });

  it("names why the push was refused off the agent's own push in the transcript, for the strip to say", async () => {
    const held = await doneOnce({ answers: { read: UNPUSHED }, says: pushRefused(DENIED) });
    const r = await settled(held);
    expect(r.sub!.result!.pushRefused).toBe("Permission to acme/web.git denied to wsp-bot");
    const row = (await held.rt.sessions.list()).find(s => s.threadId === held.threadId)!;
    expect(row.sub!.result!.pushRefused).toBe("Permission to acme/web.git denied to wsp-bot");
  });

  it("names no cause where the last push went through, or the work is all pushed", async () => {
    const through: Says = [
      ...pushRefused(DENIED),
      { kind: "tool_use", toolName: "Bash", toolUseId: "push-2", text: JSON.stringify({ command: "git push" }) },
      { kind: "tool_result", toolName: "Bash", toolUseId: "push-2", text: "Everything up-to-date" },
    ];
    const held = await doneOnce({ answers: { read: UNPUSHED }, says: through });
    expect((await settled(held)).sub!.result!.pushRefused).toBeUndefined();
    const pushed = await doneOnce({ answers: { read: PUSHED }, says: pushRefused(DENIED) });
    expect((await settled(pushed)).sub!.result!.pushRefused).toBeUndefined();
  });

  it("arms the delete for a sub-thread that never fetched the code, since nothing git holds is left", async () => {
    const held = await doneOnce({ answers: { read: "norepo\n" } });
    const r = await settled(held);
    expect(r.sub!.result).toMatchObject({ norepo: true, unpushed: 0, changed: 0, pushedSha: null });
    expect(r.sub!.deleteAt).toBeDefined();
    expect(r.sub!.held).toBeUndefined();
  });

  it("treats a read that fails as work not pushed: the machine is held, never armed for its delete", async () => {
    const held = await doneOnce({ answers: { read: { exitCode: 128, stderr: "fatal: not a git repository" } } });
    const r = await settled(held);
    expect(r.sub!.result!.readFailed).toBe("exit 128: fatal: not a git repository");
    expect(r.sub!.held).toMatchObject({ files: 0, commits: 0 });
    expect(r.sub!.deleteAt).toBeUndefined();
  });

  it("leaves a machine whose nap Boat refused running, with Boat's own stop timer at deleteAt + 10 minutes", async () => {
    const { clock } = jumpClock();
    const held = await leadOnThisComputer({ clock });
    open.push(held.rt);
    answering(held.boat, { read: PUSHED, session: null });
    const backstops: { until: number; exact: boolean }[] = [];
    held.boat.lifecycle.backstop = async (_m, at, x) => void backstops.push({ until: at, exact: x?.exact === true });
    const create = held.boat.create.bind(held.boat);
    held.boat.create = async spec => {
      const m = (await create(spec)) as StubMachine;
      m.pause = async () => {
        throw new Error("the provider will not stop this machine while the snapshot of its disk fails");
      };
      return m;
    };
    const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
    await (await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", turnToken: held.token, cwd: made.sub!.folder }, held.lead)).finished;
    const r = await until(async () => {
      const got = (await held.store.get("workspaces", made.id)) as WorkspaceRecord;
      return backstops.some(b => b.exact) ? got : undefined;
    }, "the stop timer");
    expect(r.phase).toBe("running");
    expect(backstops.filter(b => b.exact)).toEqual([{ until: r.sub!.deleteAt! + 10 * MIN, exact: true }]);
  });
});

describe("the delete at deleteAt", () => {
  it("deletes the napped box at Boat with no wake, and keeps the record released with its thread, transcript, result and session", async () => {
    const { clock, jump } = jumpClock();
    const held = await doneOnce({ clock });
    const before = await settled(held);
    const transcript = (await held.rt.sessions.history(held.made.id)).length;
    jump(31 * MIN);
    const r = await until(async () => ((await held.record()).phase === "released" ? held.record() : undefined), "the release");
    expect(held.machine.killed).toBe(true);
    expect(held.machine.resumes).toBe(0);
    expect(r.machineId).toBe("");
    expect(r.sub!.releasedAt).toBeDefined();
    expect(r.sub!.deleteAt).toBeUndefined();
    expect(r.sub!.result).toEqual(before.sub!.result);
    expect(r.sub!.session).toEqual(before.sub!.session);
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toEqual(held.answers.session);
    expect((await held.rt.sessions.history(held.made.id)).length).toBe(transcript);
    const row = (await held.rt.sessions.list()).find(s => s.threadId === held.threadId)!;
    expect(row.sub).toMatchObject({ state: "released", releasedAt: r.sub!.releasedAt, result: before.sub!.result });
    // There is no machine to wake: a send makes one.
    const wake = await held.rt.workspaces.wake(held.made.id).then(
      () => undefined,
      (e: unknown) => e as Error,
    );
    expect(wake?.message).toBe(subReleasedRefusal(held.threadId, "Boat").message);
    // A released sub-thread still goes with a delete, its cloud asked nothing, its kept session with it.
    const asked = held.boat.machines.length;
    await held.rt.sessions.delete(held.threadId);
    expect(await held.store.get("workspaces", held.made.id)).toBeUndefined();
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toBeUndefined();
    expect(held.boat.machines).toHaveLength(asked);
  });

  it("is cancelled by a turn that starts on the thread, and armed again at that turn's own done", async () => {
    const { clock, jump } = jumpClock();
    const held = await doneOnce({ clock });
    await settled(held);
    await held.rt.workspaces.wake(held.made.id);
    held.turns.hold = true;
    const next = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "one more thing", harness: "claude" });
    await until(async () => ((await held.record()).sub!.deleteAt === undefined ? true : undefined), "the delete cleared");
    jump(31 * MIN);
    await new Promise(r => setTimeout(r, 50));
    expect(held.machine.killed).toBe(false);
    expect((await held.record()).phase).not.toBe("released");
    held.turns.release!();
    await next.finished;
    const again = await until(async () => (await held.record()).sub!.deleteAt, "the delete armed again");
    expect(again).toBeGreaterThan(clock.now() + 29 * MIN);
  });

  it("turns into a hold where the machine still runs and its read now finds work not pushed", async () => {
    const { clock, jump } = jumpClock();
    const held = await leadOnThisComputer({ clock });
    open.push(held.rt);
    const answers: Answers = { read: PUSHED, session: null };
    answering(held.boat, answers);
    const create = held.boat.create.bind(held.boat);
    held.boat.create = async spec => {
      const m = (await create(spec)) as StubMachine;
      m.pause = async () => {
        throw new Error("the stop passed its 10 minute budget");
      };
      return m;
    };
    const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
    await (await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", turnToken: held.token, cwd: made.sub!.folder }, held.lead)).finished;
    const record = async (): Promise<WorkspaceRecord> => (await held.store.get("workspaces", made.id)) as WorkspaceRecord;
    await until(async () => (await record()).sub?.deleteAt, "the delete armed");
    answers.read = UNPUSHED;
    jump(31 * MIN);
    const r = await until(async () => ((await record()).sub?.held !== undefined ? record() : undefined), "the hold");
    expect(r.sub!.deleteAt).toBeUndefined();
    expect(r.phase).toBe("running");
    expect(held.boat.machines.at(-1)!.killed).toBe(false);
  });

  it("survives a restart, and a deleteAt passed while the host was down deletes at boot", async () => {
    const held = await doneOnce();
    const r = await settled(held);
    await held.rt.close();
    // The host was down when the delete was due.
    await held.store.put("workspaces", r.id, { ...r, sub: { ...r.sub!, deleteAt: Date.now() - MIN } });
    const again = held.restart();
    open.push(again);
    await again.workspaces.list();
    const after = await until(async () => ((await held.record()).phase === "released" ? held.record() : undefined), "the release at boot");
    expect(held.machine.killed).toBe(true);
    expect(after.sub!.result).toEqual(r.sub!.result);
    expect((await again.sessions.list()).find(s => s.threadId === held.threadId)?.sub?.state).toBe("released");
  });

  it("keeps the done's word when a start a restart failed ends on the napped machine with no turn run since", async () => {
    const held = await doneOnce();
    const r = await settled(held);
    await held.rt.close();
    await held.store.put("held-starts", "turn-held", { workspaceId: r.id, threadId: held.threadId, turnId: "turn-held", sessionId: "turn-held", harness: "claude", place: "box" });
    const again = held.restart();
    open.push(again);
    await again.workspaces.list();
    await new Promise(res => setTimeout(res, 100));
    const after = await held.record();
    expect(after.sub!.result).toEqual(r.sub!.result);
    expect(after.sub!.held).toBeUndefined();
    expect(after.sub!.deleteAt).toBe(r.sub!.deleteAt);
  });

  it("arms a deleteAt still ahead again at boot, and fires it then", async () => {
    const held = await doneOnce();
    await settled(held);
    await held.rt.close();
    // The restarted host's own clock: nothing the closed one armed can fire on it.
    const { clock, jump } = jumpClock();
    const again = held.restart(clock);
    open.push(again);
    await again.workspaces.list();
    expect(held.machine.killed).toBe(false);
    jump(31 * MIN);
    await until(async () => ((await held.record()).phase === "released" ? true : undefined), "the release after the restart");
    expect(held.machine.killed).toBe(true);
  });
});

describe("a box Boat lost", () => {
  it("reads released, the thread and its transcript kept", async () => {
    const held = await doneOnce();
    await settled(held);
    held.machine.killed = true;
    await held.rt.workspaces.wake(held.made.id).catch(() => undefined);
    const r = await until(async () => ((await held.record()).phase === "released" ? held.record() : undefined), "the release");
    expect(r.sub!.lost).toBeDefined();
    expect(r.gone).toBeUndefined();
    expect((await held.rt.sessions.list()).find(s => s.threadId === held.threadId)?.sub?.state).toBe("released");
    expect((await held.rt.sessions.history(held.made.id)).length).toBeGreaterThan(0);
  });
});

describe("wsp delete <sub-thread>", () => {
  it("takes the thread, its machine at Boat and its kept session", async () => {
    const held = await doneOnce();
    await settled(held);
    const gone = await held.rt.sessions.delete(held.threadId);
    expect(gone).toEqual({ workspaceId: held.made.id, threads: 1, machine: "deleted", on: "Boat" });
    expect(held.machine.killed).toBe(true);
    expect(await held.store.get("workspaces", held.made.id)).toBeUndefined();
    expect(await held.store.getBlob(SUB_SESSIONS, held.threadId)).toBeUndefined();
    expect((await held.rt.sessions.list()).some(s => s.threadId === held.threadId)).toBe(false);
  });

  it("refuses a lead's token over work not pushed, naming it, and lets the person delete it", async () => {
    const held = await doneOnce({ answers: { read: UNPUSHED } });
    await settled(held);
    const e = await held.rt.sessions.delete(held.threadId, held.lead).then(
      () => undefined,
      (x: unknown) => x as Error,
    );
    expect(e?.message).toBe(subHeldDeleteRefusal(held.threadId, { files: 3, commits: 2 }, BRANCH).message);
    expect(e?.message).toBe(`sub-thread ${held.threadId.slice(0, 8)} holds 2 commits and 3 changed files not pushed on ${BRANCH}. Send it a push, or ask the person to delete it.`);
    expect(exitClassOf(e)).toBe("usage");
    expect(held.machine.killed).toBe(false);
    await held.rt.sessions.delete(held.threadId);
    expect(held.machine.killed).toBe(true);
  });

  it("lets a lead's token delete one whose work is all pushed", async () => {
    const held = await doneOnce();
    await settled(held);
    await held.rt.sessions.delete(held.threadId, held.lead);
    expect(held.machine.killed).toBe(true);
  });
});

describe("removing Boat", () => {
  it("leaves every sub-thread released with cloudGone and its transcript, and a send says Boat left", async () => {
    const held = await leadOnThisComputer({ spendPerDayUsd: 5 });
    open.push(held.rt);
    answering(held.boat, { read: PUSHED, session: null });
    const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
    const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", turnToken: held.token, cwd: made.sub!.folder }, held.lead);
    const threadId = child.view().threadId!;
    await child.finished;
    const record = async (): Promise<WorkspaceRecord> => (await held.store.get("workspaces", made.id)) as WorkspaceRecord;
    await until(async () => (await record()).sub?.deleteAt, "the done");
    const events = (await held.rt.sessions.history(made.id)).length;
    const answer = await held.rt.places!.remove("box");
    expect(answer).toMatchObject({ removed: true });
    const r = await record();
    expect(r.phase).toBe("released");
    expect(r.sub!.cloudGone).toBe(true);
    expect(held.boat.machines.at(-1)!.killed).toBe(true);
    expect((await held.rt.sessions.history(made.id)).length).toBe(events);
    expect((await held.rt.sessions.list()).find(s => s.threadId === threadId)?.sub).toMatchObject({ state: "released", cloudGone: true });
    const asLead: Caller = held.lead;
    const send = await held.rt.sessions.start(made.id, { thread: threadId, prompt: "go on", harness: "claude", turnToken: held.token }, asLead).then(
      () => undefined,
      (e: unknown) => e as Error,
    );
    expect(send?.message).toBe(subCloudGoneRefusal("Boat").message);
    expect(send?.message).toBe("Boat is no longer on this host. Add it with wsp add boat and send again.");
    expect((await held.rt.workspaces.wake(made.id).then(() => undefined, (e: unknown) => e as Error))?.message).toBe(send?.message);
  });
});

describe("a sub-thread's delete, checked first", () => {
  it("answers the question with the work not pushed and deletes nothing", async () => {
    const held = await doneOnce({ answers: { read: UNPUSHED } });
    await settled(held);
    const asked = await held.rt.sessions.delete(held.threadId, undefined, { check: true });
    expect(asked).toEqual({
      workspaceId: held.made.id,
      threads: 1,
      on: "Boat",
      question: `Delete sub-thread ${held.threadId.slice(0, 8)} (${NAME}) and its machine on Boat?\n2 commits and 3 changed files on ${BRANCH} are not pushed and go with it.`,
    });
    expect(held.machine.killed).toBe(false);
    expect(await held.store.get("workspaces", held.made.id)).toBeDefined();
  });
});

const sleep = (ms: number): Promise<void> => new Promise(res => setTimeout(res, ms));
const refusedBy = (p: Promise<unknown>): Promise<Error | undefined> => p.then(
  () => undefined,
  (e: unknown) => e as Error,
);

describe("a delete in the seconds a follow-up turn's done still reads", () => {
  /** Done once over work not pushed, then a follow-up turn ended with the machine's session read slowed to 800 ms. */
  async function followedUp() {
    const held = await doneOnce({ answers: { read: UNPUSHED } });
    await settled(held);
    const read = held.machine.readBytes!.bind(held.machine);
    held.machine.readBytes = async (path, o) => (await sleep(800), read(path, o));
    await held.rt.workspaces.wake(held.made.id);
    const next = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "push it", harness: "claude" });
    await next.finished;
    return held;
  }

  it("refuses a lead's token at once, the turn not read yet", async () => {
    const held = await followedUp();
    const e = await refusedBy(held.rt.sessions.delete(held.threadId, held.lead));
    expect(e?.message).toBe(subHeldDeleteRefusal(held.threadId, { files: 0, commits: 0, unread: true }, BRANCH).message);
    expect(held.machine.killed).toBe(false);
  });

  it("waits for the done in flight, then refuses a lead's token over the work it found, and the person's question names it", async () => {
    const held = await followedUp();
    await sleep(50);
    const e = await refusedBy(held.rt.sessions.delete(held.threadId, held.lead));
    expect(e?.message).toBe(subHeldDeleteRefusal(held.threadId, { files: 3, commits: 2 }, BRANCH).message);
    expect((await held.rt.sessions.delete(held.threadId, undefined, { check: true })).question).toContain(`2 commits and 3 changed files on ${BRANCH} are not pushed`);
    expect(held.machine.killed).toBe(false);
    expect(await held.store.get("workspaces", held.made.id)).toBeDefined();
  });
});

describe("a restart that cut a done short", () => {
  it("runs the done at boot, so the machine is armed for its delete or held", async () => {
    const held = await leadOnThisComputer();
    open.push(held.rt);
    answering(held.boat, { read: PUSHED, session: Buffer.from("s") });
    const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
    const machine = held.boat.machines.at(-1)!;
    const read = machine.readBytes!.bind(machine);
    machine.readBytes = () => new Promise<Buffer>(() => {});
    await (await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", turnToken: held.token, cwd: made.sub!.folder }, held.lead)).finished;
    await sleep(100);
    const record = async (): Promise<WorkspaceRecord> => (await held.store.get("workspaces", made.id)) as WorkspaceRecord;
    expect((await record()).sub!.result).toBeUndefined();
    await held.rt.close();
    machine.readBytes = read;
    const again = held.restart();
    open.push(again);
    await again.workspaces.list();
    const r = await until(async () => ((await record()).sub?.deleteAt !== undefined ? record() : undefined), "the done at boot");
    expect(r.sub!.result).toMatchObject({ pushedSha: expect.any(String), unpushed: 0 });
    expect(r.phase).toBe("napping");
  });
});

describe("the agents switch over a lead's delete", () => {
  it("refuses a lead in a folder made with agents off", async () => {
    const held = await doneOnce();
    await settled(held);
    await held.rt.close();
    const folder = (await held.store.get("workspaces", held.folder.id)) as WorkspaceRecord;
    await held.store.put("workspaces", folder.id, { ...folder, agents: { ...folder.agents, spawn: false } });
    const again = held.restart();
    open.push(again);
    await again.workspaces.list();
    const e = await refusedBy(again.sessions.delete(held.threadId, held.lead));
    expect(e?.message).toContain("may not delete a sub-thread it started");
    expect(held.machine.killed).toBe(false);
  });
});

describe("what goes with Boat", () => {
  it("names each sub-thread whose machine holds work not pushed", async () => {
    const held = await doneOnce({ answers: { read: UNPUSHED }, deleteAfterMs: 30 * MIN });
    await settled(held);
    const holds = await held.rt.places!.holds("box");
    expect(holds.unsaved).toEqual([`sub-thread ${held.threadId.slice(0, 8)} holds 2 commits and 3 changed files not pushed on ${BRANCH}`]);
  });

  it("forgetting Boat keeps each sub-thread's phase, marks it cloudGone, and arms no delete", async () => {
    const { clock, jump } = jumpClock();
    const held = await doneOnce({ clock, deleteAfterMs: 30 * MIN });
    await settled(held);
    await held.rt.places!.remove("box", { forget: true });
    const r = await held.record();
    expect(r.phase).toBe("napping");
    expect(r.sub!.cloudGone).toBe(true);
    expect(r.sub!.deleteAt).toBeUndefined();
    expect((await held.rt.sessions.list()).find(s => s.threadId === held.threadId)?.sub).toMatchObject({ state: "napping", cloudGone: true });
    const e = await refusedBy(held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "go on", harness: "claude" }));
    expect(e?.message).toBe(subCloudGoneRefusal("Boat").message);
    jump(31 * MIN);
    await sleep(50);
    expect((await held.record()).phase).toBe("napping");
    await held.rt.sessions.delete(held.threadId);
    expect(await held.store.get("workspaces", held.made.id)).toBeUndefined();
  });

  it("reads a record from before the machine's rate was kept", async () => {
    const held = await doneOnce();
    const r = await settled(held);
    await held.rt.close();
    const { rateUsdPerHour: _, ...older } = r.sub!;
    await held.store.put("workspaces", r.id, { ...r, sub: older });
    const again = held.restart();
    open.push(again);
    const row = (await again.sessions.list()).find(s => s.threadId === held.threadId)!;
    expect(row.sub).toBeDefined();
    expect("rateUsdPerHour" in row.sub!).toBe(false);
  });
});

describe("a delete due while a wake runs", () => {
  it("waits for the wake and reads the machine again before it deletes", async () => {
    const { clock, jump } = jumpClock();
    const held = await doneOnce({ clock });
    await settled(held);
    const resume = held.machine.resume.bind(held.machine);
    let woke!: () => void;
    held.machine.resume = async () => {
      await new Promise<void>(res => (woke = res));
      return resume();
    };
    const waking = held.rt.workspaces.wake(held.made.id).catch(() => undefined);
    await until(() => (woke !== undefined ? true : undefined), "the wake asking");
    held.answers.read = UNPUSHED;
    jump(31 * MIN);
    await sleep(50);
    woke();
    await waking;
    const r = await until(async () => ((await held.record()).sub?.held !== undefined ? held.record() : undefined), "the hold");
    expect(held.machine.killed).toBe(false);
    expect(r.phase).not.toBe("released");
  });

  it("keeps the machine of a turn the send after the wake starts while the machine is read", async () => {
    const { clock, jump } = jumpClock();
    const held = await doneOnce({ clock });
    await settled(held);
    const resume = held.machine.resume.bind(held.machine);
    let woke!: () => void;
    held.machine.resume = async () => {
      await new Promise<void>(res => (woke = res));
      return resume();
    };
    const read = held.machine.readBytes!.bind(held.machine);
    held.machine.readBytes = async (path, o) => (await sleep(800), read(path, o));
    const waking = held.rt.workspaces.wake(held.made.id).catch(() => undefined);
    await until(() => (woke !== undefined ? true : undefined), "the wake asking");
    jump(31 * MIN);
    await sleep(50);
    woke();
    await waking;
    held.turns.hold = true;
    const next = await held.rt.sessions.start(held.made.id, { thread: held.threadId, prompt: "one more thing", harness: "claude" });
    await sleep(1_500);
    expect(held.machine.killed).toBe(false);
    expect(next.view().status).toBe("running");
    held.turns.release?.();
  });
});
