// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread held over work not pushed, on a cloud that deletes a paused
// machine's disk after a while: the record is released at that day, the box
// deleted, with one line on its row and its timeline; a turn since the nap
// moves the day on, and a host started after the day releases it at boot.
import { afterEach, describe, expect, it } from "vitest";
import { subOpening } from "@wsp/protocol";
import type { Runtime } from "../src/runtime.js";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { UNPUSHED, answering, jumpClock, leadOnThisComputer, until } from "./sub-fixture.js";

const NAME = "Fix the flaky upload test";
const BRANCH = "wsp/fix-the-flaky-upload-test";
const MIN = 60_000;
const DAY = 24 * 60 * MIN;
const KEPT = 14 * DAY;
const utcDay = (at: number): string => new Date(at).toISOString().slice(0, 10);
const keepRanOut = (at: number): string =>
  `Boat deletes a paused machine's disk 14 days after it napped, so wsp deleted this machine on ${utcDay(at)} UTC with 2 commits and 3 changed files on ${BRANCH} not pushed; the next send starts a new machine from the image`;

let open: Runtime[] = [];
afterEach(async () => {
  for (const rt of open) await rt.close().catch(() => undefined);
  open = [];
});

const settle = (): Promise<void> => new Promise(r => setTimeout(r, 50));

/** A sub-thread on Boat whose turn ended with 2 commits and 3 files not pushed, held and napping. */
async function heldNapping() {
  const { clock, jump } = jumpClock();
  const held = await leadOnThisComputer({ clock });
  open.push(held.rt);
  held.boat.lifecycle.diskKeptMs = KEPT;
  answering(held.boat, { read: UNPUSHED, session: Buffer.from("gzipped session bytes") });
  const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
  const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", notify: ["me"], turnToken: held.token, title: NAME, cwd: made.sub!.folder }, held.lead);
  const machine = held.boat.machines.at(-1)!;
  const kidId = child.view().threadId!;
  const notices: string[] = [];
  held.rt.events.on("*", e => {
    if (e.type === "session.notice" && e.threadId === kidId) notices.push(e.text);
  });
  await child.finished;
  const record = (): Promise<WorkspaceRecord> => held.store.get("workspaces", made.id) as Promise<WorkspaceRecord>;
  await until(async () => ((await record()).phase === "napping" ? true : undefined), "the nap");
  return { ...held, clock, jump, made, machine, kidId, notices, record };
}

describe("a held sub-thread on a cloud that deletes a paused disk", () => {
  it("is released at the day the keep runs out, its box deleted, with one line on its row and its timeline", async () => {
    const held = await heldNapping();
    const ends = (await held.record()).nappedAt! + KEPT;
    held.jump(ends - held.clock.now() - MIN);
    await settle();
    expect((await held.record()).phase).toBe("napping");
    expect((await held.record()).sub!.held).toMatchObject({ files: 3, commits: 2 });
    expect(held.machine.killed).not.toBe(true);
    held.jump(2 * MIN);
    const r = await until(async () => ((await held.record()).phase === "released" ? held.record() : undefined), "the release");
    const line = keepRanOut(ends);
    expect(r.machineId).toBe("");
    expect(r.sub!.held).toBeUndefined();
    expect(r.sub!.lost).toBe(line);
    expect(r.diskGone).toBe(line);
    expect(held.machine.killed).toBe(true);
    expect(held.notices).toEqual([line]);
    expect((await held.rt.sessions.list()).filter(s => s.threadId === held.kidId).at(-1)!.diskGone).toBe(line);
  });

  it("a turn since the nap counts the keep from the next nap", async () => {
    const held = await heldNapping();
    held.jump(10 * DAY);
    await held.rt.workspaces.wake(held.made.id);
    await (await held.rt.sessions.start(held.made.id, { thread: held.kidId, prompt: "Push it.", harness: "claude" })).finished;
    const first = (await held.record()).nappedAt!;
    const again = await until(async () => {
      const r = await held.record();
      return r.phase === "napping" && r.nappedAt! > first ? r : undefined;
    }, "the second nap");
    held.jump(5 * DAY);
    await settle();
    expect((await held.record()).phase).toBe("napping");
    held.jump(again.nappedAt! + KEPT - held.clock.now());
    const r = await until(async () => ((await held.record()).phase === "released" ? held.record() : undefined), "the release");
    expect(r.sub!.lost).toBe(keepRanOut(again.nappedAt! + KEPT));
  });

  it("a host started after the day releases it at boot, counting from the cloud's own stop", async () => {
    const held = await heldNapping();
    const stop = held.clock.now() - 15 * DAY;
    held.machine.pausedAt = new Date(stop).toISOString();
    await held.rt.close();
    open = open.filter(rt => rt !== held.rt);
    const again = held.restart();
    open.push(again);
    await again.sessions.list();
    const r = await until(async () => ((await held.record()).phase === "released" ? held.record() : undefined), "the release at boot");
    expect(r.sub!.lost).toBe(keepRanOut(stop + KEPT));
    expect(held.machine.killed).toBe(true);
  });
});
