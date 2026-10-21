// SPDX-License-Identifier: AGPL-3.0-only
// Ctrl+Tab and Ctrl+Shift+Tab open the next and the previous thread at the open
// thread's own level of the sidebar, in the order it draws them: the live
// top-level threads, the settled ones, or one lead's sub-threads. Each press
// opens the thread at once, with no overlay, in its composer, with its row
// scrolled into view; a held key steps on every repeat; the ends wrap. A
// thread in Needs you and in its tree steps by its place in the tree. In a
// terminal or on the right panel's tab strip the pair stays theirs.
import { act, cleanup, configure, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES, type SessionView, type WorkspaceView } from "@wsp/protocol";
import type { Api } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { useRightPanelStore } from "../src/rightPanelStore.js";
import { AppShell } from "../src/shell/AppShell.js";
import { onComposerFocusRequest } from "../src/shell/shellRequests.js";
import { useTerminalDrawerStore } from "../src/terminal/drawerStore.js";
import { caps } from "./caps.js";
import { noDaemonApi } from "./fake-daemon-api.js";
import { clearNotices } from "./notice-text.js";

const CAPS = caps();
const NOW = Date.now();
const MIN = 60_000;

const view = (id: string, createdAt: string): WorkspaceView => ({
  id,
  name: id,
  machineId: `m_${id}`,
  project: { id: "pr_1", name: "acme-lab", path: "/root", computer: "default" },
  phase: "running",
  golden: "snap_g",
  createdAt,
});

/** A thread that ended a while ago and has been read, so it stands in the live list until it settles. */
const done = (threadId: string, workspaceId: string, minutesAgo: number, over: Partial<SessionView> = {}): SessionView => ({
  id: `s_${threadId}`,
  workspaceId,
  threadId,
  harness: "claude",
  status: "completed",
  prompt: `Work on ${threadId}.`,
  startedBy: "person",
  startedAt: NOW - minutesAgo * MIN,
  endedAt: NOW - minutesAgo * MIN + MIN,
  readAt: NOW - minutesAgo * MIN + 2 * MIN,
  ...over,
});
const running = (threadId: string, parentThreadId: string, minutesAgo: number): SessionView => {
  const { endedAt: _endedAt, ...row } = done(threadId, "ws_a", minutesAgo, { status: "running", parentThreadId, startedBy: "agent" });
  return row;
};

const WORKSPACES = [view("ws_a", "2026-09-01T01:00:00Z"), view("ws_b", "2026-09-01T02:00:00Z")];
const SESSIONS: SessionView[] = [
  done("top_1", "ws_a", 10),
  done("top_2", "ws_b", 20),
  done("lead", "ws_a", 30),
  running("sub_1", "lead", 25),
  running("sub_2", "lead", 24),
  running("sub_3", "lead", 23),
  running("deep_1", "sub_2", 22),
  running("deep_2", "sub_2", 19),
  { ...done("sub_failed", "ws_a", 21, { status: "failed", parentThreadId: "lead", startedBy: "agent" }), readAt: undefined },
  // Ended and read, so it stands in the lead's shut Finished fold, a level a live sub-thread never steps into.
  done("sub_done", "ws_a", 26, { parentThreadId: "lead", startedBy: "agent" }),
  done("top_3", "ws_b", 40),
  done("settled_1", "ws_a", 50, { settledAt: NOW - 5 * MIN }),
  done("settled_2", "ws_b", 60, { settledAt: NOW - 6 * MIN }),
  done("settled_3", "ws_a", 70, { settledAt: NOW - 7 * MIN }),
  // Read five hours ago, past the two hours a read thread sits quiet, so it settled on its own and leaves Settled
  // only while some other thread holds it there.
  done("settled_quiet", "ws_b", 300),
];
/** Rows one case adds to the host's list. */
let extra: SessionView[] = [];

function fakeApi(): Api {
  return {
    listWorkspaces: async () => WORKSPACES,
    getWorkspace: async id => WORKSPACES.find(w => w.id === id)!,
    createWorkspace: async () => WORKSPACES[0]!,
    watchStatuses: async () => [],
    forget: async () => {},
    nap: async id => WORKSPACES.find(w => w.id === id)!,
    wake: async id => WORKSPACES.find(w => w.id === id)!,
    capabilities: async () => CAPS,
    startSession: async o => ({ id: "s9", workspaceId: o.workspaceId, harness: "claude", status: "running" }),
    portReach: async (_id, port) => ({ url: `https://m1-${port}.preview.example/?pt_token=e`, expiresAt: Date.now() + 3_600_000 }),
    daemon: noDaemonApi,
    sessionHistory: async () => [],
    listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
    snapshotStorage: async () => null,
    rollbackSnapshot: async () => ({ lineage: { name: "default", head: null, versions: [] }, existingWorkspaces: "untouched" }),
    listSessions: async () => [...SESSIONS, ...extra],
    subscribe: () => () => {},
    getGolden: async () => undefined,
  };
}

const press = (mods: { shiftKey?: boolean; repeat?: boolean } = {}, at: Element | Window = document.activeElement ?? window) =>
  act(() => {
    fireEvent.keyDown(at, { key: "Tab", code: "Tab", ctrlKey: true, ...mods });
  });
const openThread = () => useStore.getState().selectedThreadId;
const open = (workspaceId: string, threadId: string) => act(() => useStore.getState().select(workspaceId, threadId));
const workspaceOf = (threadId: string): string => SESSIONS.find(s => s.threadId === threadId)!.workspaceId;
const settle = () => act(() => new Promise<void>(resolve => setTimeout(resolve, 20)));

/** The thread rows the sidebar draws in the order it draws them, the Needs you inbox's copies left out. */
const drawnRows = (): string[] =>
  [...document.querySelectorAll<HTMLElement>("[data-sidebar-tree] [data-row-id^='thread:']")].map(row => row.dataset["rowId"]!.slice("thread:".length));
const inOrderDrawn = (ids: readonly string[]): string[] => drawnRows().filter(id => ids.includes(id));

/** Presses the chord once per thread at the level and reads where each press lands. */
async function walk(from: string, presses: number, mods: { shiftKey?: boolean; repeat?: boolean } = {}): Promise<string[]> {
  open(workspaceOf(from), from);
  const landed: string[] = [];
  for (let i = 0; i < presses; i++) {
    press(i === 0 ? { shiftKey: mods.shiftKey ?? false } : mods);
    landed.push(openThread()!);
  }
  await act(async () => {});
  return landed;
}

vi.setConfig({ testTimeout: 15_000 });
configure({ asyncUtilTimeout: 10_000 });

beforeEach(() => {
  window.localStorage.clear();
  // The Settled section stands open, so its rows are drawn.
  window.localStorage.setItem("wsp:sidebar-folded", JSON.stringify([]));
  vi.spyOn(navigator, "platform", "get").mockReturnValue("Linux x86_64");
  window.wsp = {};
  useStore.setState({ api: null, conn: "live", capabilities: null, workspaces: [], statuses: {}, costs: {}, spending: {}, selectedId: null, selectedThreadId: null, creations: [], sessions: {}, ready: false, preferences: { ...DEFAULT_PREFERENCES, labs: true } });
  clearNotices();
  useRightPanelStore.setState({ byWorkspaceId: {} });
  useTerminalDrawerStore.setState({ byWorkspaceId: {} });
});

afterEach(() => {
  extra = [];
  cleanup();
  delete window.wsp;
  vi.restoreAllMocks();
});

async function mountShell(): Promise<void> {
  useStore.getState().bind(fakeApi());
  render(
    <AppShell>
      <div>center content</div>
    </AppShell>,
  );
  await waitFor(() => expect(Object.values(useStore.getState().sessions).flat()).toHaveLength(SESSIONS.length + extra.length));
  await waitFor(() => expect(drawnRows()).toContain("settled_3"));
}

describe("the Ctrl+Tab step through the sidebar's threads", () => {
  it("steps a top-level thread through the live top-level threads as drawn, skipping sub-threads and Settled, wrapping both ways", async () => {
    await mountShell();
    const tops = inOrderDrawn(["top_1", "top_2", "lead", "top_3"]);
    expect(tops).toHaveLength(4);
    const from = tops[0]!;
    expect(await walk(from, 4)).toEqual([...tops.slice(1), from]);
    expect(await walk(from, 4, { shiftKey: true })).toEqual([...tops.slice(1).reverse(), from]);
  });

  it("steps a settled top-level thread through the settled top-level threads only, one that settled on its own included", async () => {
    await mountShell();
    const settled = inOrderDrawn(["settled_1", "settled_2", "settled_3", "settled_quiet"]);
    expect(settled).toHaveLength(4);
    for (const from of [settled[0]!, "settled_quiet"]) {
      const at = settled.indexOf(from);
      const after = [...settled.slice(at + 1), ...settled.slice(0, at + 1)];
      expect(await walk(from, 4)).toEqual(after);
      expect(await walk(from, 4, { shiftKey: true })).toEqual([...after.slice(0, -1).reverse(), from]);
    }
  });

  it("steps a sub-thread through its lead's live sub-threads as drawn, never its own sub-threads, the lead nor the shut Finished fold", async () => {
    await mountShell();
    expect(drawnRows()).not.toContain("sub_done");
    expect(document.querySelector(`[data-row-id='finished:thread:lead']`)).not.toBeNull();
    const siblings = inOrderDrawn(["sub_1", "sub_2", "sub_3", "sub_failed"]);
    expect(siblings).toHaveLength(4);
    const from = siblings[0]!;
    expect(await walk(from, 4)).toEqual([...siblings.slice(1), from]);
    expect(await walk(from, 4, { shiftKey: true })).toEqual([...siblings.slice(1).reverse(), from]);
    const deep = inOrderDrawn(["deep_1", "deep_2"]);
    expect(deep).toHaveLength(2);
    expect(await walk(deep[0]!, 2)).toEqual([deep[1], deep[0]]);
  });

  it("steps a thread shown in Needs you and in its tree by its place in the tree", async () => {
    await mountShell();
    expect(document.querySelector("[data-row-id='inbox:thread:sub_failed']")).not.toBeNull();
    const siblings = inOrderDrawn(["sub_1", "sub_2", "sub_3", "sub_failed"]);
    const at = siblings.indexOf("sub_failed");
    open("ws_a", "sub_failed");
    press();
    expect(openThread()).toBe(siblings[(at + 1) % siblings.length]);
  });

  it("passes over a thread whose row a folded section hides, and onto one a folded section copies but its tree draws", async () => {
    extra = [{ ...done("top_failed", "ws_b", 15, { status: "failed" }), readAt: undefined }];
    window.localStorage.setItem("wsp:sidebar-folded", JSON.stringify(["needs-you"]));
    await mountShell();
    expect(drawnRows()).not.toContain("top_failed");
    const tops = inOrderDrawn(["top_1", "top_2", "lead", "top_3"]);
    expect(await walk(tops[0]!, 4)).toEqual([...tops.slice(1), tops[0]]);
    const siblings = inOrderDrawn(["sub_1", "sub_2", "sub_3", "sub_failed"]);
    expect(siblings).toHaveLength(4);
    expect(await walk(siblings[0]!, 4)).toEqual([...siblings.slice(1), siblings[0]]);
  });

  it("opens each step at once with no overlay, steps again on every repeat of a held key, and lets nothing up commit a walk", async () => {
    await mountShell();
    const tops = inOrderDrawn(["top_1", "top_2", "lead", "top_3"]);
    open(workspaceOf(tops[0]!), tops[0]!);
    press();
    expect(openThread()).toBe(tops[1]);
    press({ repeat: true });
    expect(openThread()).toBe(tops[2]);
    press({ repeat: true });
    expect(openThread()).toBe(tops[3]);
    await settle();
    expect(document.querySelector("[data-workspace-switcher]")).toBeNull();
    expect(document.querySelector("[role='listbox'][aria-label='Task switcher']")).toBeNull();
    act(() => {
      fireEvent.keyUp(window, { key: "Control" });
    });
    expect(openThread()).toBe(tops[3]);
  });

  it("puts the caret in the opened thread's composer and scrolls its row into view in the sidebar", async () => {
    await mountShell();
    const scrolled: string[] = [];
    vi.spyOn(Element.prototype, "scrollIntoView").mockImplementation(function (this: Element) {
      scrolled.push(this.getAttribute("data-row-id") ?? "");
    });
    const tops = inOrderDrawn(["top_1", "top_2", "lead", "top_3"]);
    open(workspaceOf(tops[0]!), tops[0]!);
    const next = tops[1]!;
    const asks: string[] = [];
    const off = onComposerFocusRequest(workspaceOf(next), () => asks.push(next));
    try {
      press();
      await waitFor(() => expect(scrolled).toContain(`thread:${next}`));
      expect(asks).toContain(next);
    } finally {
      off();
    }
  });

  it("leaves the pair to a focused terminal and to the right panel's tab strip", async () => {
    await mountShell();
    open("ws_a", "top_1");
    const term = document.createElement("div");
    term.dataset["terminalOwner"] = "drawer";
    const input = document.createElement("textarea");
    term.appendChild(input);
    document.body.appendChild(term);
    try {
      act(() => input.focus());
      press({}, input);
      expect(openThread()).toBe("top_1");
    } finally {
      term.remove();
    }
    act(() => {
      useRightPanelStore.getState().open("ws_a", "machine");
      useRightPanelStore.getState().open("ws_a", "processes");
    });
    await waitFor(() => expect(document.querySelectorAll("[data-right-panel-tab-list] [data-active-tab]")).toHaveLength(2));
    act(() => document.querySelector<HTMLElement>("[data-right-panel-tab-list] [data-active-tab='true'] button:last-of-type")!.focus());
    press();
    expect(openThread()).toBe("top_1");
  });
});
