// SPDX-License-Identifier: AGPL-3.0-only
// Every thread of a project folder shares one workspace record, so the right
// panel keys its pane by the thread on screen: what one thread opened is not in
// another's pane, a new thread opens with the pane shut, and only the browser
// guests of the three threads shown last stay loaded.
import { act, cleanup } from "@testing-library/react";
import type { WorkspaceView } from "@wsp/protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { guestKey, resetGuests, showGuest, useGuests, type Guest } from "../src/browser/guests.js";
import { resetBrowserTabs, useBrowserTabs } from "../src/browser/tabs.js";
import { LAST_WORKSPACE_KEY } from "../src/protocol/lastWorkspace.js";
import { useStore } from "../src/protocol/store.js";
import { openPanelTerminal } from "../src/shell/shellCommands.js";
import { provideTerminals, WorkspaceTerminals } from "../src/terminal/link.js";
import {
  carryNewThreadPanel,
  followThreadPanels,
  keepListedPanels,
  KEPT_THREAD_PANELS,
  selectPanelTerminalIds,
  selectWorkspaceRightPanelState,
  threadPanelKey,
  useRightPanelStore,
} from "../src/rightPanelStore.js";

const WS = "ws_folder";
const KEY = "wsp:right-panel-state:v1";
const RECT = { x: 0, y: 0, width: 400, height: 300, raised: false };
const view: WorkspaceView = { id: WS, name: "lab", machineId: "m_lab", project: { id: "pr_lab", name: "lab", path: "/Users/dev/lab", computer: "default" }, phase: "running", golden: "snap_g", createdAt: "2026-09-01T00:00:00Z" };

const show = (threadId: string | null, fresh = false): void =>
  act(() => useStore.setState({ selectedId: WS, selectedThreadId: threadId, freshThread: fresh, settingsOpen: false }));
const shown = () => selectWorkspaceRightPanelState(useRightPanelStore.getState().byWorkspaceId, WS);
const kinds = () => shown().surfaces.map(s => s.kind);
const tabOf = (): string => {
  const surface = shown().surfaces.find(s => s.kind === "preview");
  if (surface?.kind !== "preview" || surface.resourceId === null) throw new Error("no browser tab in the shown pane");
  return surface.resourceId;
};
/** The guest a shown browser tab makes, on the place its tab is at. */
const madeFor = (tabId: string): Guest => {
  const tab = useBrowserTabs.getState().byWorkspaceId[WS]![tabId]!;
  const at = tab.entries[tab.index];
  return { workspaceId: WS, tabId, src: at && "url" in at ? at.url : "about:blank", route: null, loaded: tab.loads };
};

let stops: (() => void)[] = [];

beforeEach(() => {
  window.localStorage.clear();
  resetGuests();
  resetBrowserTabs();
  useRightPanelStore.setState({ byWorkspaceId: {} });
  useStore.setState({ ready: true, workspaces: [view], creations: [], statuses: {}, sessions: {}, selectedId: null, selectedThreadId: null, freshThread: false, settingsOpen: false });
  stops = [followThreadPanels()];
});

afterEach(() => {
  for (const stop of stops) stop();
  cleanup();
  provideTerminals(WS, null);
  resetGuests();
  useStore.setState({ selectedId: null, selectedThreadId: null, freshThread: false, ready: false, workspaces: [] });
});

describe("the right pane belongs to the thread on screen", () => {
  it("a browser tab and the Agents panel opened in thread A are not in thread B's pane, and are back on A with Agents in front", () => {
    show("th_a");
    const store = useRightPanelStore.getState();
    store.openNewBrowser(WS);
    store.open(WS, "agents");
    expect(kinds()).toEqual(["preview", "agents"]);

    show("th_b");
    expect(kinds()).toEqual([]);
    expect(shown().isOpen).toBe(false);

    show("th_a");
    expect(kinds()).toEqual(["preview", "agents"]);
    expect(shown().activeSurfaceId).toBe("agents");
    expect(shown().isOpen).toBe(true);
  });

  it("what thread B opens stays in B, so switching back and forth never shows the two sets together", () => {
    show("th_a");
    useRightPanelStore.getState().open(WS, "agents");
    show("th_b");
    useRightPanelStore.getState().open(WS, "processes");
    show("th_a");
    expect(kinds()).toEqual(["agents"]);
    show("th_b");
    expect(kinds()).toEqual(["processes"]);
  });

  it("a new thread opens with the pane shut; what it opened before its first send stays with the thread the send made", () => {
    show("th_a");
    useRightPanelStore.getState().open(WS, "agents");

    show(null, true);
    expect(shown().isOpen).toBe(false);
    expect(kinds()).toEqual([]);
    useRightPanelStore.getState().open(WS, "files");

    act(() => {
      carryNewThreadPanel(WS, "th_new");
      useStore.getState().readingThread(WS, "th_new");
    });
    expect(useStore.getState().selectedThreadId).toBe("th_new");
    expect(kinds()).toEqual(["files"]);
    expect(shown().isOpen).toBe(true);

    show(null, true);
    expect(shown().isOpen).toBe(false);
    expect(kinds()).toEqual([]);
  });

  it("a thread clicked from the New thread page gets none of what the page had open", () => {
    show(null, true);
    useRightPanelStore.getState().open(WS, "agents");
    show("th_other");
    expect(kinds()).toEqual([]);
    expect(shown().isOpen).toBe(false);
  });

  it("a thread nobody has opened anything in, a fork or one an agent started, opens with the pane shut", () => {
    show("th_never_seen");
    expect(shown()).toEqual({ isOpen: false, activeSurfaceId: null, surfaces: [] });
  });

  it("a pty one thread's pane holds is never the drawer's, whichever thread is on screen", () => {
    show("th_a");
    useRightPanelStore.getState().openTerminal(WS, "p1");
    show("th_b");
    useRightPanelStore.getState().openTerminal(WS, "p2");
    expect(selectPanelTerminalIds(useRightPanelStore.getState().byWorkspaceId, WS).sort()).toEqual(["p1", "p2"]);
  });

  it("a terminal asked for in thread A whose pty lands after a switch to B opens in A's pane, not B's", async () => {
    let answer = (): void => {};
    const answered = new Promise<void>(resolve => (answer = resolve));
    const wt = new WorkspaceTerminals({ request: async op => (op === "pty.create" ? (await answered, { ok: true, ptyId: "p7" }) : { ok: true, ptys: [] }) });
    wt.feedStatus("live");
    provideTerminals(WS, wt);
    show("th_a");
    const opened = openPanelTerminal(WS);
    show("th_b");
    answer();
    await act(() => opened);
    expect(shown()).toEqual({ isOpen: false, activeSurfaceId: null, surfaces: [] });
    expect(useRightPanelStore.getState().byWorkspaceId[threadPanelKey(WS, "th_a")]?.activeSurfaceId).toBe("terminal:p7");
  });

  it("a pty the daemon no longer holds leaves every thread's pane, not only the one on screen", () => {
    show("th_a");
    useRightPanelStore.getState().openTerminal(WS, "p1");
    show("th_b");
    useRightPanelStore.getState().reconcileTerminalSurfaces(WS, []);
    expect(useRightPanelStore.getState().byWorkspaceId[threadPanelKey(WS, "th_a")]?.surfaces).toEqual([]);
  });

  it("keeps the panes of the threads written to last, up to its cap, so a forgotten thread's pane does not stay for good", () => {
    for (let n = 0; n <= KEPT_THREAD_PANELS; n++) {
      show(`th_${n}`);
      useRightPanelStore.getState().open(WS, "agents");
    }
    show("th_0");
    useRightPanelStore.getState().open(WS, "diff");
    const keys = Object.keys(useRightPanelStore.getState().byWorkspaceId);
    expect(keys).toHaveLength(KEPT_THREAD_PANELS);
    expect(keys).not.toContain(threadPanelKey(WS, "th_1"));
    expect(keys.at(-1)).toBe(threadPanelKey(WS, "th_0"));
  });

  it("a workspace the host stops listing takes every one of its threads' panes with it", () => {
    show("th_a");
    useRightPanelStore.getState().open(WS, "agents");
    show("th_b");
    useRightPanelStore.getState().open(WS, "diff");
    stops.push(keepListedPanels());
    expect(Object.keys(useRightPanelStore.getState().byWorkspaceId).sort()).toEqual([threadPanelKey(WS, "th_a"), threadPanelKey(WS, "th_b")]);
    act(() => useStore.setState({ workspaces: [] }));
    expect(useRightPanelStore.getState().byWorkspaceId).toEqual({});
  });
});

describe("browser guests across threads", () => {
  it("with five threads that each opened a browser tab, the guests of the three shown last stay loaded, and the first one's tab loads its place again when shown", () => {
    const tabs: Record<string, string> = {};
    for (const n of [1, 2, 3, 4, 5]) {
      show(`th_${n}`);
      useRightPanelStore.getState().openBrowser(WS, useBrowserTabs.getState().createTab(WS, { url: `https://example.com/${n}` }));
      tabs[n] = tabOf();
      showGuest(guestKey(WS, tabs[n]!), madeFor(tabs[n]!), RECT);
    }
    const held = () => Object.keys(useGuests.getState().guests).sort();
    expect(held()).toEqual([3, 4, 5].map(n => guestKey(WS, tabs[n]!)).sort());

    show("th_1");
    expect(tabOf()).toBe(tabs[1]);
    showGuest(guestKey(WS, tabs[1]!), madeFor(tabs[1]!), RECT);
    expect(useGuests.getState().guests[guestKey(WS, tabs[1]!)]?.src).toBe("https://example.com/1");
    expect(held()).toEqual([1, 4, 5].map(n => guestKey(WS, tabs[n]!)).sort());
  });

  it("New thread, as a project's home or a folder's page, takes none of the three places, so the threads shown before it keep their guests", () => {
    const tabs: string[] = [];
    for (const n of [1, 2, 3]) {
      show(`th_${n}`);
      useRightPanelStore.getState().openBrowser(WS, useBrowserTabs.getState().createTab(WS, { url: `https://example.com/${n}` }));
      tabs.push(tabOf());
      showGuest(guestKey(WS, tabOf()), madeFor(tabOf()), RECT);
    }
    act(() => useStore.setState({ selectedId: null, selectedThreadId: null, freshThread: false }));
    show(null, true);
    show("th_2");
    expect(Object.keys(useGuests.getState().guests).sort()).toEqual(tabs.map(tab => guestKey(WS, tab)).sort());
  });

  it("a browser tab made after a reload never takes the id of a tab saved in another thread's pane", async () => {
    vi.resetModules();
    const before = await import("../src/browser/tabs.js");
    const saved = before.useBrowserTabs.getState().createTab(WS, null);
    vi.resetModules();
    const reloaded = await import("../src/browser/tabs.js");
    expect(reloaded.useBrowserTabs.getState().createTab(WS, null)).not.toBe(saved);
  });
});

describe("a pane saved by a build that kept one per workspace", () => {
  it("goes to the thread that was open; every other thread starts shut, and another folder's terminal goes back to its drawer", async () => {
    const shell = { id: "terminal:p5", kind: "terminal", resourceId: "p5", terminalIds: ["p5"], activeTerminalId: "p5" };
    window.localStorage.setItem(LAST_WORKSPACE_KEY, JSON.stringify({ "": { workspaceId: WS, threadId: "th_open" } }));
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ state: { byWorkspaceId: { [WS]: { isOpen: true, activeSurfaceId: "agents", surfaces: [{ id: "agents", kind: "agents" }] }, ws_other: { isOpen: true, activeSurfaceId: "terminal:p5", surfaces: [shell] } } }, version: 1 }),
    );
    await useRightPanelStore.persist.rehydrate();
    show("th_open");
    expect(kinds()).toEqual(["agents"]);
    expect(shown().isOpen).toBe(true);
    show("th_other");
    expect(shown().isOpen).toBe(false);
    expect(kinds()).toEqual([]);
    expect(useRightPanelStore.getState().byWorkspaceId["ws_other"]).toBeUndefined();
    expect(selectPanelTerminalIds(useRightPanelStore.getState().byWorkspaceId, "ws_other")).toEqual([]);
  });
});
