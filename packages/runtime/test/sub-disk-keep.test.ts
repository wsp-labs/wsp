// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread held over work not pushed, on a cloud that may delete a paused
// machine's disk after a while: its row carries that day, and nothing is
// released or deleted when it comes, since a Boat box stopped 18 days still
// resumed whole; a wake that finds the disk gone says so (sub-resume).
import { afterEach, describe, expect, it } from "vitest";
import { subOpening } from "@wsp/protocol";
import type { Runtime } from "../src/runtime.js";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { UNPUSHED, answering, jumpClock, leadOnThisComputer, until } from "./sub-fixture.js";

const NAME = "Fix the flaky upload test";
const MIN = 60_000;
const DAY = 24 * 60 * MIN;
const KEPT = 14 * DAY;

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

describe("a held sub-thread on a cloud that may delete a paused disk", () => {
  it("is neither released nor deleted at the day the keep runs out, and its row keeps carrying that day", async () => {
    const held = await heldNapping();
    const ends = (await held.record()).nappedAt! + KEPT;
    held.jump(ends - held.clock.now() + DAY);
    await settle();
    const r = await held.record();
    expect(r.phase).toBe("napping");
    expect(r.sub!.held).toMatchObject({ files: 3, commits: 2 });
    expect(r.sub!.lost).toBeUndefined();
    expect(r.diskGone).toBeUndefined();
    expect(held.machine.killed).not.toBe(true);
    expect(held.notices).toEqual([]);
    expect((await held.rt.sessions.list()).filter(s => s.threadId === held.kidId).at(-1)!.diskKeptUntil).toBe(ends);
  });

  it("a host started after the day keeps the record and the box, counting the day from the cloud's own stop", async () => {
    const held = await heldNapping();
    const stop = held.clock.now() - 15 * DAY;
    held.machine.pausedAt = new Date(stop).toISOString();
    await held.rt.close();
    open = open.filter(rt => rt !== held.rt);
    const again = held.restart();
    open.push(again);
    const row = await until(async () => (await again.sessions.list()).find(s => s.threadId === held.kidId && s.diskKeptUntil !== undefined), "the row with its keep");
    expect(row.diskKeptUntil).toBe(stop + KEPT);
    await settle();
    const r = await held.record();
    expect(r.phase).toBe("napping");
    expect(r.sub!.held).toMatchObject({ files: 3, commits: 2 });
    expect(held.machine.killed).not.toBe(true);
  });
});
