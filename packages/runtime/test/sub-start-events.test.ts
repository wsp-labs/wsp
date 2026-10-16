// SPDX-License-Identifier: AGPL-3.0-only
// What a window hears of a sub-thread's start, in order: the record under its
// lead before the cloud is asked for a machine, its thread held under the lead
// with the start's steps, and every event and read after it naming the lead.
// A window files a record that comes without its lead at the sidebar's root.
import { afterEach, describe, expect, it } from "vitest";
import { subOpening, type SessionView, type WorkspaceView } from "@wsp/protocol";
import type { Runtime } from "../src/runtime.js";
import { CONNECT_TIMED_OUT, connectTimedOut, leadOnThisComputer, until } from "./sub-fixture.js";

let open: Runtime[] = [];
afterEach(async () => {
  for (const rt of open) await rt.close().catch(() => undefined);
  open = [];
});

type Heard = { type: string; workspaceId?: string; workspace?: WorkspaceView; askedBy?: { threadId: string }; row?: SessionView; facts?: { parentThreadId?: string } };

describe("a sub-thread's start, as a window hears it", () => {
  it("names the lead in every event and read from the first, which is the record, before the cloud is asked for a machine", async () => {
    const { rt, boat, project, lead, threadId, token } = await leadOnThisComputer({ picks: [".env"] });
    const heard: { e: Heard; creates: number }[] = [];
    rt.events.on("*", e => void heard.push({ e: structuredClone(e) as Heard, creates: boat.creates.length }));
    let readsAtHold: Promise<[SessionView[], WorkspaceView[]]> | undefined;
    let createsAtHold: number | undefined;
    rt.events.on("*", e => {
      if (e.type !== "session.held" || readsAtHold !== undefined) return;
      createsAtHold = boat.creates.length;
      readsAtHold = Promise.all([rt.sessions.list(), rt.workspaces.list()]);
    });

    const made = await rt.workspaces.create({ project: project.id, name: "Kid", sub: { on: "boat" } }, lead);
    expect(made).toMatchObject({ parentThreadId: threadId, rootThreadId: threadId });
    const child = await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Run the suite."), harness: made.sub!.harness, turnToken: token, cwd: made.sub!.folder }, lead);
    await child.finished;

    const own = heard.filter(({ e }) => e.workspaceId === made.id || e.workspace?.id === made.id);
    // The record comes first, with its lead, while Boat has made nothing.
    expect(own[0]).toMatchObject({ e: { type: "workspace.created", workspace: { id: made.id, parentThreadId: threadId, rootThreadId: threadId } }, creates: 0 });
    // Its thread's row, with the start's steps, comes before any stage of the machine and before Boat is asked.
    const firstRow = own.findIndex(({ e }) => e.type === "session.row" && e.row !== undefined);
    expect(own[firstRow]).toMatchObject({ e: { row: { threadId: child.view().threadId, parentThreadId: threadId, sub: { spawn: { branch: { branch: "feat/upload" } } } } }, creates: 0 });
    expect(own.findIndex(({ e }) => e.type === "workspace.creating")).toBeGreaterThan(firstRow);
    for (const { e } of own) {
      if (e.type === "workspace.creating") expect(e.askedBy?.threadId).toBe(threadId);
      if (e.type === "workspace.created") expect(e.workspace?.parentThreadId).toBe(threadId);
      if (e.type === "session.row" && e.row !== undefined) expect(e.row.parentThreadId).toBe(threadId);
      if (e.type === "thread.head") expect(e.facts?.parentThreadId).toBe(threadId);
    }
    // Its thread is held under the lead before the machine is asked for, and its row carries the start's steps.
    expect(createsAtHold).toBe(0);
    const [rows, workspaces] = await readsAtHold!;
    const row = rows.find(r => r.workspaceId === made.id);
    expect(row).toMatchObject({ parentThreadId: threadId, rootThreadId: threadId, sub: { spawn: { branch: { branch: "feat/upload" } } } });
    expect(workspaces.find(w => w.id === made.id)).toMatchObject({ parentThreadId: threadId });
    expect(boat.creates).toHaveLength(1);
  });

  it("replays no stage of its machine to a window that subscribes while it is made: the record and the row say it", async () => {
    const { rt, boat, project, lead, token } = await leadOnThisComputer();
    open.push(rt);
    let land!: () => void;
    const landed = new Promise<void>(r => (land = r));
    const create = boat.create.bind(boat);
    boat.create = async spec => (await landed, create(spec));
    const staged = new Set<string>();
    rt.events.on("*", e => void (e.type === "workspace.creating" && staged.add(e.workspaceId)));
    const made = await rt.workspaces.create({ project: project.id, name: "Kid", sub: { on: "boat" } }, lead);
    const child = rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Run the suite."), harness: made.sub!.harness, turnToken: token, cwd: made.sub!.folder }, lead);
    await until(() => staged.has(made.id), "a stage of the machine");
    expect(rt.workspaces.creating().filter(s => s.workspaceId === made.id)).toEqual([]);
    land();
    await (await child).finished;
  });

  it("answers a window's reads of its thread while its machine is made, and the reads after it hold the turn", async () => {
    const { rt, boat, project, lead, token } = await leadOnThisComputer();
    open.push(rt);
    let land!: () => void;
    const landed = new Promise<void>(r => (land = r));
    let making!: () => void;
    const made$ = new Promise<void>(r => (making = r));
    const exec = boat.execImpl;
    boat.execImpl = async (m, cmd, stdin) => (cmd.startsWith("install -d ") ? (making(), await landed, exec(m, cmd, stdin)) : exec(m, cmd, stdin));
    const made = await rt.workspaces.create({ project: project.id, name: "Kid", sub: { on: "boat" } }, lead);
    const child = rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Run the suite."), harness: made.sub!.harness, turnToken: token, cwd: made.sub!.folder }, lead);
    await made$;
    const threadId = (await rt.sessions.list()).find(r => r.workspaceId === made.id)!.threadId!;
    // The window's reads, as the app asks them: no thread names the caller.
    expect(await rt.sessions.history(made.id)).toEqual([]);
    expect(await rt.sessions.page(made.id, { threadId })).toMatchObject({ events: [] });
    expect(await rt.sessions.threadHistory(made.id, { threadId })).toEqual([]);
    expect(await rt.sessions.head(threadId)).toMatchObject({ facts: { threadId, workspaceId: made.id }, events: [] });
    land();
    await (await child).finished;
    const types = (await rt.sessions.page(made.id, { threadId })).events.map(e => e.type);
    expect(types).toContain("session.start");
    expect(types).toContain("session.end");
  });

  it("keeps a first start that failed on its row under the lead, saying why, through a restart, and Try again sends the first message as it was", async () => {
    const { rt, boat, project, lead, threadId, token, restart, starts } = await leadOnThisComputer();
    open.push(rt);
    boat.faults = [{ kind: "unusable", replayed: true }, { kind: "unusable" }];
    const made = await rt.workspaces.create({ project: project.id, name: "Kid", sub: { on: "boat" } }, lead);
    const opening = subOpening(made.sub!.preamble, "Run the suite.");
    const refused = await rt.sessions.start(made.id, { prompt: opening, harness: made.sub!.harness, turnToken: token, cwd: made.sub!.folder }, lead).then(
      () => undefined,
      (e: unknown) => e as Error,
    );
    expect(refused?.message).toMatch(/^Boat gave two machines in a row/);
    const failed = async (at: Runtime) => {
      const row = (await at.sessions.list()).find(r => r.workspaceId === made.id);
      expect(row).toMatchObject({ status: "failed", parentThreadId: threadId, rootThreadId: threadId, failure: expect.stringMatching(/^Boat gave two machines in a row/), sub: { state: "released", spawn: { failed: { step: "machine" } } } });
      expect((await at.workspaces.list()).find(w => w.id === made.id)).toMatchObject({ phase: "released", parentThreadId: threadId });
      return row!;
    };
    const row = await failed(rt);
    expect(row.sub!.spawn!.asked).toBe(opening);
    await rt.close();
    const again = restart();
    open.push(again);
    await failed(again);
    // Try again sends the row's message, which is the first one: it opens on a new machine with no second preamble.
    const tried = await again.sessions.start(made.id, { thread: row.threadId!, prompt: opening, harness: "claude", cwd: made.sub!.folder }, lead);
    await tried.finished;
    expect(starts.at(-1)!.o.prompt).toBe(opening);
    expect(boat.creates).toHaveLength(3);
  });

  it("opens a send into a first start that failed with that start's preamble and task, whatever the send's words, and keeps the first message for Try again", async () => {
    const { rt, boat, project, lead, token, starts } = await leadOnThisComputer();
    open.push(rt);
    boat.faults = [{ kind: "unusable", replayed: true }, { kind: "unusable" }];
    const made = await rt.workspaces.create({ project: project.id, name: "Kid", sub: { on: "boat" } }, lead);
    const opening = subOpening(made.sub!.preamble, "Run the suite.");
    await rt.sessions.start(made.id, { prompt: opening, harness: made.sub!.harness, turnToken: token, cwd: made.sub!.folder }, lead).catch(() => undefined);
    const threadId = (await rt.sessions.list()).find(r => r.workspaceId === made.id)!.threadId!;
    const asked = async (): Promise<string | undefined> => (await rt.sessions.list()).find(r => r.workspaceId === made.id)!.sub!.spawn!.asked;
    // A second start that fails the same way keeps the first message as the one Try again sends.
    boat.faults = [{ kind: "unusable", replayed: true }, { kind: "unusable" }];
    await rt.sessions.start(made.id, { thread: threadId, prompt: "Try once more, the suite is flaky.", harness: "claude" }, lead).catch(() => undefined);
    expect(await asked()).toBe(opening);
    const sent = await rt.sessions.start(made.id, { thread: threadId, prompt: "Try once more, the suite is flaky.", harness: "claude" }, lead);
    await sent.finished;
    expect(starts.at(-1)!.o.prompt).toBe(subOpening(opening, "Try once more, the suite is flaky."));
    expect(starts.at(-1)!.o.prompt.startsWith(made.sub!.preamble)).toBe(true);
    expect(starts.at(-1)!.o.prompt).toContain("Run the suite.");
  });

  it("names the addresses its connect tried on the row and in its lead's line, when Boat never answered its first start", async () => {
    const f = await leadOnThisComputer();
    open.push(f.rt);
    f.boat.create = async () => {
      throw connectTimedOut();
    };
    const made = await f.rt.workspaces.create({ project: f.project.id, name: "Kid", sub: { on: "boat" } }, f.lead);
    await f.endLead();
    await f.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Run the suite."), harness: made.sub!.harness, notify: [f.threadId], turnToken: f.token, cwd: made.sub!.folder }, f.lead).catch(() => undefined);
    const row = (await f.rt.sessions.list()).find(r => r.workspaceId === made.id);
    expect(row?.failure).toContain(`fetch failed: ${CONNECT_TIMED_OUT}`);
    await expect.poll(() => f.starts.some(s => s.o.prompt.includes(`fetch failed: ${CONNECT_TIMED_OUT}`))).toBe(true);
  });
});
