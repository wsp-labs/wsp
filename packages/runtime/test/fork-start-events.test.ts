// SPDX-License-Identifier: AGPL-3.0-only
// What a window hears of a fork a thread asked for, in order: the record under
// the thread that asked before the first stage of its machine, and every event
// and read after it naming that thread. A window files a stage that comes with
// no record behind it as a create at the sidebar's root, and moves it under its
// lead only once the record lands.
import { randomUUID } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { AGENTS_ON, type Caller, type ThreadScope, type TurnResult, type WorkspaceCreatingEvent, type WorkspaceView } from "@wsp/protocol";
import { copyKey, createRuntime, type HarnessAdapterFactory, type Runtime } from "../src/runtime.js";
import { memoryStore, type Store } from "../src/store.js";
import { newPlaceKeyPair } from "../src/places.js";
import { branchDaemons, createOn, fakeLocal, stubBackend, withDaemonRoads, type StubBackend } from "./stub-backend.js";
import { until } from "./until.js";

const IMAGE = {
  head: 1,
  versions: [{ version: 1, snapshotId: "snap_image-v1", kind: "desktop", baseTemplate: "base", setupSha: "abc", createdAt: "2026-09-01T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 } }],
};

/** A lead's turn that runs until the test ends it, as a thread's turn runs while its fork is made. */
function heldTurns(): { factory: HarnessAdapterFactory; end: () => void } {
  const ends: (() => void)[] = [];
  const factory: HarnessAdapterFactory = () => ({
    steers: false,
    start: ({ onEvent }) => {
      const sessionId = randomUUID();
      const result: TurnResult = { status: "completed", text: "done" };
      let end!: () => void;
      const finished = new Promise<TurnResult>(resolve => {
        end = () => {
          onEvent({ type: "turn.done", sessionId, result });
          onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
          resolve(result);
        };
      });
      ends.push(end);
      onEvent({ type: "session.start", sessionId });
      return { localId: sessionId, finished, interrupt: async () => end() };
    },
  });
  return { factory, end: () => ends.forEach(e => e()) };
}

type Heard = { type: string; workspaceId?: string; workspace?: WorkspaceView; askedBy?: { threadId: string }; stage?: string };

describe("a fork a thread asked for, as a window hears it", () => {
  let root: string;
  let store: Store;
  let backend: StubBackend;
  let rt: Runtime;
  let turns: ReturnType<typeof heldTurns>;
  /** Whether the lead's copy reads as holding commits on the default branch, which refuses its fork. */
  let onDefault: boolean;

  beforeEach(async () => {
    root = mkdtempSync(join(tmpdir(), "wsp-fork-start-"));
    store = memoryStore();
    await store.put("goldens", copyKey("default", "default"), IMAGE);
    backend = withDaemonRoads(stubBackend());
    turns = heldTurns();
    onDefault = false;
    rt = createRuntime({
      backend,
      store,
      adapters: { claude: turns.factory },
      daemonToken: "cafef00d".repeat(3),
      daemonChannel: branchDaemons({ branchOf: () => (onDefault ? "main" : "work"), aheadOf: () => (onDefault ? 1 : 0), defaultOf: () => "main" }).open,
      local: { ...fakeLocal(root), daemonRoad: async () => ({ url: "ws://this-computer", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "cafef00d".repeat(3) }) },
      placeLinks: { hostKey: newPlaceKeyPair(), provider: () => undefined, here: () => ({ name: "this-mac" }), hostName: () => "this-mac" },
    });
  });
  afterEach(async () => {
    turns.end();
    await rt.close().catch(() => undefined);
    rmSync(root, { recursive: true, force: true });
  });

  const leadThread = async (): Promise<{ threadId: string; asked: Caller }> => {
    const lead = await createOn(rt, { golden: "snap_g", name: "lead", agents: AGENTS_ON });
    const threadId = (await rt.sessions.start(lead.id, { prompt: "lead" })).view().threadId!;
    const scope: ThreadScope = { kind: "thread", threadId, workspaceId: lead.id, rootThreadId: threadId };
    return { threadId, asked: { origin: "relayed", by: scope } };
  };

  it("names its lead in every event and read from the first, which is the record, before the machine is asked for", async () => {
    const { threadId, asked } = await leadThread();
    let land!: () => void;
    const landed = new Promise<void>(r => (land = r));
    const make = backend.create.bind(backend);
    backend.create = async spec => (await landed, make(spec));
    const heard: { e: Heard; machines: number }[] = [];
    rt.events.on("*", e => void heard.push({ e: structuredClone(e) as Heard, machines: backend.machines.length }));
    let readsAtStage: Promise<[WorkspaceView[], WorkspaceCreatingEvent[]]> | undefined;
    rt.events.on("workspace.creating", e => {
      if (readsAtStage === undefined && (e as WorkspaceCreatingEvent).name === "builder") readsAtStage = Promise.all([rt.workspaces.list(), rt.workspaces.creating()]);
    });

    const making = createOn(rt, { name: "builder" }, asked);
    await until(() => readsAtStage !== undefined);
    const [listed, replayed] = await readsAtStage!;
    land();
    const made = await making;
    expect(made).toMatchObject({ parentThreadId: threadId, rootThreadId: threadId, phase: "running" });

    const own = heard.filter(({ e }) => e.workspaceId === made.id || e.workspace?.id === made.id);
    // The record comes first, under its lead, while only the lead's machine stands.
    expect(own[0]).toMatchObject({ e: { type: "workspace.created", workspace: { id: made.id, name: "builder", parentThreadId: threadId, rootThreadId: threadId } }, machines: 1 });
    expect(own.some(({ e }) => e.type === "workspace.creating")).toBe(true);
    for (const { e } of own) {
      if (e.type === "workspace.creating") expect(e.askedBy?.threadId).toBe(threadId);
      if (e.type === "workspace.created") expect(e.workspace?.parentThreadId).toBe(threadId);
    }
    expect(own.filter(({ e }) => e.type === "workspace.created").at(-1)).toMatchObject({ e: { workspace: { id: made.id, phase: "running", machineId: made.machineId } } });
    // A window that lists while the machine is made holds it under its lead, and is replayed no stage it would
    // file at the root.
    expect(listed.find(w => w.id === made.id)).toMatchObject({ parentThreadId: threadId, rootThreadId: threadId });
    expect(replayed.filter(s => s.workspaceId === made.id)).toEqual([]);
    expect((await rt.workspaces.list()).filter(w => w.id === made.id)).toEqual([made].map(({ notice: _, ...v }) => expect.objectContaining({ ...v, phase: "running" })));
  });

  it("keeps a fork whose machine failed under its lead, gone with why, and lists it until the person deletes it", async () => {
    const { threadId, asked } = await leadThread();
    backend.create = async () => {
      throw Object.assign(new Error("the provider refused the fork"), { kind: "invalid" });
    };
    const heard: Heard[] = [];
    rt.events.on("*", e => void heard.push(structuredClone(e) as Heard));
    await expect(createOn(rt, { name: "builder" }, asked)).rejects.toThrow("the provider refused the fork");
    const id = heard.find(e => e.type === "workspace.created" && e.workspace?.name === "builder")!.workspace!.id;
    const own = heard.filter(e => e.workspaceId === id || e.workspace?.id === id);
    expect(own.map(e => e.stage ?? `${e.type}:${e.workspace?.phase ?? ""}`)).toEqual(["workspace.created:queued", "fork-requested", "workspace.created:gone", "failed"]);
    const failed = { id, phase: "gone", machineId: "", gone: expect.stringContaining("the provider refused the fork"), parentThreadId: threadId, rootThreadId: threadId };
    expect(own.at(-2)!.workspace).toMatchObject(failed);
    expect(own.at(-1)!.askedBy?.threadId).toBe(threadId);
    // A window that lists after the failure holds it under its lead with why, until the person deletes it.
    expect((await rt.workspaces.list()).find(w => w.id === id)).toMatchObject(failed);
    await rt.workspaces.delete(id);
    expect(heard.at(-1)).toMatchObject({ type: "workspace.deleted", workspaceId: id });
    expect((await rt.workspaces.list()).some(w => w.id === id)).toBe(false);
  });

  it("shows nothing of a fork its lead's branch refuses, which is read before any stage", async () => {
    const { asked } = await leadThread();
    onDefault = true;
    const heard: Heard[] = [];
    rt.events.on("*", e => void heard.push(structuredClone(e) as Heard));
    await expect(createOn(rt, { name: "builder" }, asked)).rejects.toMatchObject({ kind: "usage" });
    expect(heard.filter(e => e.type === "workspace.created" || e.type === "workspace.deleted")).toEqual([]);
    expect(backend.machines).toHaveLength(1);
  });
});
