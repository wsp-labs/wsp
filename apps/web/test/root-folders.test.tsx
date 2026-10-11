// SPDX-License-Identifier: AGPL-3.0-only
// A workspace that holds a copy of its project works in the copy: the panes root there and a dialog about the
// working folder names it, never the project folder the copy was taken from.
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { WorkspaceView } from "@wsp/protocol";
import { carryNewThreadRoot, useRoot, useRootStore, useRoots, useWorkingFolder } from "../src/files/root.js";
import { provideDaemonHello } from "../src/files/wire.js";
import { useStore } from "../src/protocol/store.js";
import { DAEMON_HELLO, resetSurfaces } from "./surface-harness.js";

const WS = "ws_copy";
const copied: WorkspaceView = {
  id: WS,
  name: "cart",
  machineId: "local",
  phase: "running",
  golden: "",
  createdAt: "2026-09-01T00:00:00Z",
  project: { id: "pr_1", name: "spoo", path: "/Users/maya/spoo", computer: "here" },
  worktree: { path: "/Users/maya/wsp-work/spoo-cart", branch: "cart", made: true },
  folder: "/Users/maya/wsp-work/spoo-cart",
};

beforeEach(() => {
  resetSurfaces();
  provideDaemonHello(WS, DAEMON_HELLO);
  useStore.setState({ workspaces: [copied] });
});

describe("a copy's folders", () => {
  it("roots the panes at the copy, not the project folder it was taken from", () => {
    const { result } = renderHook(() => useRoots(WS));
    expect(result.current).toContain("/Users/maya/wsp-work/spoo-cart");
    expect(result.current).not.toContain("/Users/maya/spoo");
  });

  it("names the copy as the working folder before the thread reports one", () => {
    const { result } = renderHook(() => useWorkingFolder(WS));
    expect(result.current).toBe("/Users/maya/wsp-work/spoo-cart");
  });
});

describe("the panes' folder of threads that share a project folder", () => {
  const shared: WorkspaceView = { ...copied, id: "ws_shared", project: { id: "pr_2", name: "lab", path: "/root/lab", computer: "here" }, worktree: undefined, folder: undefined };

  beforeEach(() => {
    provideDaemonHello(shared.id, { root: "/root" });
    useStore.setState({ workspaces: [shared], selectedId: shared.id, selectedThreadId: "th_a", freshThread: false });
  });

  it("is a thread's own: one with no folder yet opens on the project folder, not where another thread went, and a pin stays with its thread", () => {
    const { result } = renderHook(() => useRoot(shared.id));
    act(() => useRootStore.getState().follow(shared.id, "/root/lab/api"));
    expect(result.current).toBe("/root/lab/api");
    act(() => useStore.setState({ selectedThreadId: "th_b" }));
    expect(result.current).toBe("/root/lab");
    act(() => useRootStore.getState().pin(shared.id, "/root/lab/docs"));
    expect(result.current).toBe("/root/lab/docs");
    act(() => useStore.setState({ selectedThreadId: "th_a" }));
    expect(result.current).toBe("/root/lab/api");
  });

  it("is the thread's that the New thread page's first send opens, with what the page pinned, and the next page starts clean", () => {
    act(() => useStore.setState({ selectedThreadId: null, freshThread: true }));
    const { result } = renderHook(() => useRoot(shared.id));
    act(() => useRootStore.getState().pin(shared.id, "/root/lab/docs"));
    expect(result.current).toBe("/root/lab/docs");
    act(() => {
      carryNewThreadRoot(shared.id, "th_new");
      useStore.setState({ selectedThreadId: "th_new", freshThread: false });
    });
    expect(result.current).toBe("/root/lab/docs");
    act(() => useStore.setState({ selectedThreadId: null, freshThread: true }));
    expect(result.current).toBe("/root/lab");
  });
});
