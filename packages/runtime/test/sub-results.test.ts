// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's work coming back to its lead: the finished line ends with what
// the done read off the machine, in one of three lines, and a merge takes the
// branch off the record whatever state the machine is in, waking nothing and
// reading nothing there, and says what the machine holds that is not pushed.
import { afterEach, describe, expect, it } from "vitest";
import { mergeInLine, subOpening, type ThreadHead } from "@wsp/protocol";
import type { Clock } from "../src/clock.js";
import type { Runtime } from "../src/runtime.js";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { PUSHED, UNPUSHED, answering, jumpClock, leadOnThisComputer, until, type Answers } from "./sub-fixture.js";

const NAME = "Fix the flaky upload test";
const BRANCH = "wsp/fix-the-flaky-upload-test";
const MIN = 60_000;
const DAY = 24 * 60 * MIN;
const utcDay = (at: number): string => new Date(at).toISOString().slice(0, 10);

let open: Runtime[] = [];
afterEach(async () => {
  for (const rt of open) await rt.close().catch(() => undefined);
  open = [];
});

/** A lead on this computer and its sub-thread on Boat, started with `notify`, the machine answering the result read
 * with `read`; with `hold` the sub-thread's turn runs until `release`. */
async function subStarted(o: { read: Answers["read"]; notify?: "lead" | "me"; hold?: true; clock?: Clock; diskKeptMs?: number }) {
  const held = await leadOnThisComputer(o.clock !== undefined ? { clock: o.clock } : {});
  open.push(held.rt);
  if (o.diskKeptMs !== undefined) held.boat.lifecycle.diskKeptMs = o.diskKeptMs;
  answering(held.boat, { read: o.read, session: Buffer.from("gzipped session bytes") });
  const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
  if (o.hold === true) held.turns.hold = true;
  const notify = o.notify === "me" ? ["me"] : [held.threadId];
  const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", notify, turnToken: held.token, title: NAME, cwd: made.sub!.folder }, held.lead);
  const machine = held.boat.machines.at(-1)!;
  const kidId = child.view().threadId!;
  const record = (): Promise<WorkspaceRecord> => held.store.get("workspaces", made.id) as Promise<WorkspaceRecord>;
  const merge = () => held.rt.workspaces.mergeIn({ workspaceId: held.folder.id, child: made.id, threadId: held.threadId, childThreadId: kidId }, held.lead);
  return { ...held, made, machine, kidId, kid8: kidId.slice(0, 8), lead8: held.threadId.slice(0, 8), child, record, merge };
}

/** The line the lead's next turn is handed once the sub-thread's finished line reaches it. */
async function leadLine(held: Awaited<ReturnType<typeof subStarted>>): Promise<string> {
  await held.child.finished;
  await held.endLead();
  const head = `thread ${held.kid8} finished (completed`;
  return until(() => held.starts.find(s => s.o.prompt.startsWith(head))?.o.prompt, "the line into the lead");
}

describe("a sub-thread's finished line to its lead", () => {
  it("ends with the branch pushed, the commits it brings, the merge that takes it and the delete in a span", async () => {
    const held = await subStarted({ read: PUSHED });
    const line = await leadLine(held);
    const { kid8, lead8 } = held;
    expect(line).toMatch(new RegExp(`^thread ${kid8} finished \\(completed[^)]*\\): done\\n`));
    expect(line.split("\n").at(-1)).toBe(
      `branch ${BRANCH} pushed at 9c2e1d4, 3 commits on feat/upload; merge it with wsp merge in ${lead8} ${kid8}. Its machine naps now and is deleted in 30 minutes unless you send to ${kid8} first.`,
    );
  });

  it("ends with the work not pushed and the machine kept when the read found commits and files no remote has", async () => {
    const held = await subStarted({ read: UNPUSHED });
    const { kid8 } = held;
    expect((await leadLine(held)).split("\n").at(-1)).toBe(
      `2 commits and 3 changed files on ${BRANCH} are not pushed, so its machine is kept, napping at no cost. Send to ${kid8} to have them pushed, or ask the person to delete it.`,
    );
  });

  it("names the day Boat deletes the disk of a machine kept over work not pushed", async () => {
    const held = await subStarted({ read: UNPUSHED, diskKeptMs: 14 * DAY });
    const { kid8 } = held;
    const line = (await leadLine(held)).split("\n").at(-1);
    const heldAt = (await held.record()).sub!.held!.at;
    expect(line).toBe(
      `2 commits and 3 changed files on ${BRANCH} are not pushed, so its machine naps at no cost until ${utcDay(heldAt + 14 * DAY)} UTC, when Boat deletes its disk with them. Send to ${kid8} to have them pushed, or ask the person to delete it.`,
    );
  });

  it("ends with nothing to merge when the sub-thread pushed no commit", async () => {
    const held = await subStarted({ read: "norepo\n" });
    const { kid8 } = held;
    expect((await leadLine(held)).split("\n").at(-1)).toBe(`no commits on ${BRANCH}, so there is nothing to merge. Its machine naps now and is deleted in 30 minutes unless you send to ${kid8} first.`);
  });
});

describe("a sub-thread's row and head", () => {
  it("carry its machine on Boat, and a head asked after the read carries what the done after its turn read", async () => {
    const held = await subStarted({ read: PUSHED, notify: "me" });
    await held.child.finished;
    const { facts } = await held.rt.sessions.head(held.kidId, undefined, { afterRead: true });
    expect(facts.sub).toMatchObject({ on: "boat", machine: NAME, branch: BRANCH, result: { branch: BRANCH, pushedSha: "9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e", commits: 3, unpushed: 0, changed: 0 } });
    expect(Date.parse(facts.sub!.result!.readAt)).toBeGreaterThanOrEqual(facts.endedAt!);
    const row = (await held.rt.sessions.list()).find(s => s.threadId === held.kidId)!;
    expect(row.sub?.result).toEqual(facts.sub!.result);
    expect(row.computerName).toBe("Boat");
  });

  it("a head asked with afterRead as the done is pushed, before the process exits, waits for the read after the exit", async () => {
    const held = await subStarted({ read: PUSHED, notify: "me", hold: true });
    await until(() => held.turns.release, "the sub-thread's turn");
    held.turns.exitLater = true;
    const head = new Promise<ThreadHead>(resolve => held.rt.events.on("session.done", e => void (e.type === "session.done" && e.threadId === held.kidId && resolve(held.rt.sessions.head(held.kidId, undefined, { afterRead: true })))));
    held.turns.release!();
    await new Promise(r => setTimeout(r, 50));
    held.turns.exit!();
    expect((await head).facts.sub?.result).toMatchObject({ commits: 3, unpushed: 0 });
  });
});

describe("a merge of a sub-thread into its lead", () => {
  /** What the merge asked of the lead's daemon, and that the machine was neither woken nor run on. */
  async function mergedQuietly(held: Awaited<ReturnType<typeof subStarted>>) {
    const frames = held.daemon.frames.length;
    const ran = held.machine.execLog.length;
    const resumes = held.machine.resumes;
    const phase = (await held.record()).phase;
    const done = await held.merge();
    const asked = held.daemon.frames.slice(frames);
    expect(asked.find(f => f["op"] === "git.mergeIn")).toMatchObject({ branch: BRANCH, cwd: held.repo });
    expect(asked.every(f => f["cwd"] === held.repo)).toBe(true);
    expect(held.machine.execLog.length).toBe(ran);
    expect(held.machine.resumes).toBe(resumes);
    expect((await held.record()).phase).toBe(phase);
    return done;
  }

  it("merges the record's branch while the sub-thread's machine runs", async () => {
    const held = await subStarted({ read: PUSHED, notify: "me", hold: true });
    await until(() => held.turns.release, "the sub-thread's turn");
    const done = await mergedQuietly(held);
    expect(done).toEqual({ lead: "web", child: NAME, branch: BRANCH, merged: true, commits: 3, conflicts: [] });
    held.turns.release!();
  });

  it("merges the record's branch while the machine naps, and wakes nothing", async () => {
    const held = await subStarted({ read: PUSHED, notify: "me" });
    await until(async () => (await held.record()).phase === "napping", "the nap");
    expect(held.machine.paused).toBe(true);
    const done = await mergedQuietly(held);
    expect(done).toMatchObject({ branch: BRANCH, merged: true, commits: 3 });
    expect(done.unpushed).toBeUndefined();
    expect(held.machine.paused).toBe(true);
  });

  it("merges the record's branch once the machine is deleted at Boat and the record released", async () => {
    const { clock, jump } = jumpClock();
    const held = await subStarted({ read: PUSHED, notify: "me", clock });
    await until(async () => (await held.record()).phase === "napping", "the nap");
    jump(31 * MIN);
    await until(async () => (await held.record()).phase === "released", "the release");
    expect(held.machine.killed).toBe(true);
    const creates = held.boat.creates.length;
    const done = await mergedQuietly(held);
    expect(done).toMatchObject({ branch: BRANCH, merged: true, commits: 3 });
    expect(held.boat.creates.length).toBe(creates);
  });

  it("answers the commits the machine holds unpushed and says they are not in the merge", async () => {
    const held = await subStarted({ read: UNPUSHED, notify: "me" });
    await until(async () => (await held.record()).phase === "napping", "the nap");
    const done = await mergedQuietly(held);
    expect(done).toMatchObject({ branch: BRANCH, merged: true, unpushed: 2 });
    const { kid8, lead8 } = held;
    expect(mergeInLine({ ...done, lead: `thread ${lead8}`, child: `thread ${kid8}` }, held.kidId).split("\n")).toEqual([
      `thread ${lead8}: merged ${BRANCH} from thread ${kid8}, 3 commits`,
      `${kid8} holds 2 commits not pushed on its machine; they are not in this merge.`,
    ]);
  });
});
