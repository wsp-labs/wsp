// SPDX-License-Identifier: AGPL-3.0-only
// What the host keeps per workspace for its shells' thread tags and its box
// panes' port watches goes with the workspace: a deleted one leaves no key.
import { describe, expect, it } from "vitest";
import { DAEMON_VERSION } from "@wsp/protocol";
import { channelsArea } from "../src/account/channels.js";
import { boxPanelsOpener } from "../src/account/box-panels.js";
import { eventBus } from "../src/types/events.js";
import { memoryStore } from "../src/store.js";
import type { RuntimeContext } from "../src/context.js";
import type { DaemonChannel } from "../src/daemon-channel.js";
import type { LiveWorkspace } from "../src/types/wiring.js";

const daemon = (): DaemonChannel => {
  let n = 0;
  return { send: async frame => ({ id: frame.id, ok: true, ptyId: `pty_${++n}` }) as never, close: () => {}, closed: new Promise(() => {}) };
};

describe("a deleted workspace's state", () => {
  it("leaves no shell tag key once its last tag write settles, and another workspace keeps its own", async () => {
    const bus = eventBus();
    const store = memoryStore();
    const area = channelsArea({ bus, store } as unknown as RuntimeContext);
    for (const ws of ["ws_gone", "ws_kept"]) await area.threadTagged(ws, daemon()).send({ id: null, op: "pty.create", thread: "th_a" } as never);
    await Promise.all([...area.tagWrites.values()]);
    expect([...area.tagsHeld.keys()].sort()).toEqual(["ws_gone", "ws_kept"]);
    expect(await store.get("pty-threads", "ws_gone")).toEqual({ pty_1: "th_a" });

    bus.emit({ type: "workspace.deleted", workspaceId: "ws_gone" });
    await area.tagWrites.get("ws_gone");
    await new Promise(resolve => setImmediate(resolve));

    expect(await store.get("pty-threads", "ws_gone")).toBeUndefined();
    expect([...area.tagsHeld.keys()]).toEqual(["ws_kept"]);
    expect(area.tagWrites.has("ws_gone")).toBe(false);
  });

  it("leaves no shell tag key when a shell the daemon opens lands after the delete, and writes no document for it", async () => {
    const bus = eventBus();
    const store = memoryStore();
    const area = channelsArea({ bus, store } as unknown as RuntimeContext);
    let answer: () => void = () => {};
    const slow: DaemonChannel = { ...daemon(), send: frame => new Promise(resolve => (answer = () => resolve({ id: frame.id, ok: true, ptyId: "pty_late" } as never))) };
    const opening = area.threadTagged("ws_gone", slow).send({ id: null, op: "pty.create", thread: "th_a" } as never);
    await new Promise(resolve => setImmediate(resolve));

    bus.emit({ type: "workspace.deleted", workspaceId: "ws_gone" });
    await area.tagWrites.get("ws_gone");
    answer();
    await opening;
    await area.tagWrites.get("ws_gone");
    await new Promise(resolve => setImmediate(resolve));

    expect(await store.get("pty-threads", "ws_gone")).toBeUndefined();
    expect(area.tagsHeld.size).toBe(0);
    expect(area.tagWrites.size).toBe(0);
  });

  it("leaves no port watch placement for its folder, and another folder keeps its own", async () => {
    const bus = eventBus();
    const threadRecords = new Map([
      ["th_a", { workspaceId: "ws_gone" }],
      ["th_b", { workspaceId: "ws_kept" }],
    ]);
    const placeDoor = { link: () => ({}), nameOf: () => "box", reportOf: async () => ({ daemonVersion: DAEMON_VERSION }) };
    const ctx = { bus, sessions: new Map(), threadRecords, placeDoor } as unknown as RuntimeContext;
    const placements = new Map<string, Map<string, number>>();
    const open = boxPanelsOpener(ctx, new Map(), placements);
    for (const ws of ["ws_gone", "ws_kept"]) {
      const pane = open({ record: { id: ws } } as LiveWorkspace, "pl_1", async () => ({ ok: true, ports: [] }), "/home/me/lab");
      expect(await pane.send({ op: "ports.watch", roots: [] })).toMatchObject({ ok: true });
    }
    expect([...placements.keys()].sort()).toEqual(["ws_gone", "ws_kept"]);

    bus.emit({ type: "workspace.deleted", workspaceId: "ws_gone" });

    expect([...placements.keys()]).toEqual(["ws_kept"]);
  });
});
