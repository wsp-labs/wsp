// SPDX-License-Identifier: AGPL-3.0-only
// Mod+w inside the right panel closes its open tab, the focus landing in the
// tab that takes its place so the next press closes that one; anywhere else
// it closes nothing. A browser tab's page is a webContents of its own, so the
// desktop shell hands its mod+w on as a chord, which closes the tab the same
// way. A terminal in the panel lets Command+w through and keeps Control+w,
// which is the shell's word erase.
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES, type SessionView, type WorkspaceView } from "@wsp/protocol";
import { DEFAULT_RESOLVED_KEYBINDINGS } from "../src/keybindingDefaults.js";
import { isTerminalAppShortcut } from "../src/keybindings.js";
import type { Api } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { selectWorkspaceRightPanelState, useRightPanelStore } from "../src/rightPanelStore.js";
import { AppShell } from "../src/shell/AppShell.js";
import { useTerminalDrawerStore } from "../src/terminal/drawerStore.js";
import { caps } from "./caps.js";
import { noDaemonApi } from "./fake-daemon-api.js";
import { clearNotices } from "./notice-text.js";

const CAPS = caps();
const MAC = "MacIntel";
const LINUX = "Linux x86_64";

const WORKSPACE: WorkspaceView = { id: "ws_a", name: "api", machineId: "m_a", project: { id: "pr_1", name: "the-project", path: "/root", computer: "default" }, phase: "running", golden: "snap_g", createdAt: "2026-09-01T00:00:00Z" };
const thread = (id: string, prompt: string, minutesAgo: number): SessionView => ({ id: `s_${id}`, threadId: id, workspaceId: "ws_a", harness: "claude", status: "completed", prompt, startedBy: "person", startedAt: Date.now() - minutesAgo * 60_000, endedAt: Date.now() - minutesAgo * 60_000 + 30_000 });
const THREADS = [thread("thr_1", "Fix the login redirect.", 5), thread("thr_2", "Bump the lockfile.", 10)];

function fakeApi(): Api {
  return {
    listWorkspaces: async () => [WORKSPACE],
    getWorkspace: async () => WORKSPACE,
    createWorkspace: async () => WORKSPACE,
    watchStatuses: async () => [],
    nap: async () => WORKSPACE,
    wake: async () => WORKSPACE,
    capabilities: async () => CAPS,
    startSession: async o => ({ id: "s9", workspaceId: o.workspaceId, harness: "claude", status: "running" }),
    portReach: async (_id, port) => ({ url: `https://m1-${port}.preview.example/?pt_token=e`, expiresAt: Date.now() + 3_600_000 }),
    daemon: noDaemonApi,
    sessionHistory: async () => [],
    listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
    snapshotStorage: async () => null,
    rollbackSnapshot: async () => ({ lineage: { name: "default", head: null, versions: [] }, existingWorkspaces: "untouched" }),
    listSessions: async () => THREADS,
    subscribe: () => () => {},
    getGolden: async () => undefined,
  };
}

const ctrlTab = (target: Element | Window, mods: { shiftKey?: boolean } = {}) => fireEvent.keyDown(target, { key: "Tab", code: "Tab", ctrlKey: true, ...mods });
const panel = () => selectWorkspaceRightPanelState(useRightPanelStore.getState().byWorkspaceId, "ws_a");
const content = () => document.querySelector<HTMLElement>("[data-preview-panel-mode] [data-right-panel-surface-content]");

beforeEach(() => {
  window.localStorage.clear();
  vi.spyOn(navigator, "platform", "get").mockReturnValue(MAC);
  window.wsp = {};
  useStore.setState({ api: null, conn: "live", capabilities: null, workspaces: [], statuses: {}, costs: {}, spending: {}, selectedId: null, selectedThreadId: null, creations: [], sessions: {}, ready: false, preferences: { ...DEFAULT_PREFERENCES, labs: true } });
  clearNotices();
  useRightPanelStore.setState({ byWorkspaceId: {} });
  useTerminalDrawerStore.setState({ byWorkspaceId: {} });
});

afterEach(() => {
  cleanup();
  delete window.wsp;
  vi.restoreAllMocks();
});

async function mountWithTabs(kinds: ReadonlyArray<"machine" | "processes" | "pr">): Promise<string[]> {
  useStore.getState().bind(fakeApi());
  render(
    <AppShell>
      <div>center content</div>
    </AppShell>,
  );
  await waitFor(() => expect(useStore.getState().sessions["ws_a"]?.length).toBe(2));
  act(() => useStore.getState().select("ws_a", "thr_1"));
  act(() => {
    for (const kind of kinds) useRightPanelStore.getState().open("ws_a", kind);
  });
  await waitFor(() => expect(document.querySelectorAll("[data-right-panel-tab-list] [data-active-tab]")).toHaveLength(kinds.length));
  return panel().surfaces.map(surface => surface.id);
}

const modW = (target: Element | Window, platform = MAC) => fireEvent.keyDown(target, platform === MAC ? { key: "w", code: "KeyW", metaKey: true } : { key: "w", code: "KeyW", ctrlKey: true });

describe("mod+w", () => {
  it("closes the panel's open tab with the focus inside it, then the one that took its place", async () => {
    const ids = await mountWithTabs(["machine", "processes", "pr"]);
    act(() => content()!.focus());
    modW(document.activeElement!);
    await waitFor(() => expect(panel().surfaces.map(surface => surface.id)).toEqual([ids[0], ids[1]]));
    await waitFor(() => expect(content()!.contains(document.activeElement)).toBe(true));
    modW(document.activeElement!);
    await waitFor(() => expect(panel().surfaces.map(surface => surface.id)).toEqual([ids[0]]));
  });

  it("closes the tab from the tab strip's own button too, and with Control off macOS", async () => {
    vi.spyOn(navigator, "platform", "get").mockReturnValue(LINUX);
    const ids = await mountWithTabs(["machine", "processes"]);
    act(() => document.querySelector<HTMLElement>("[data-right-panel-tab-list] [data-active-tab='true'] button")!.focus());
    modW(document.activeElement!, LINUX);
    await waitFor(() => expect(panel().surfaces.map(surface => surface.id)).toEqual([ids[0]]));
  });

  it("closes nothing with the focus outside the panel", async () => {
    const ids = await mountWithTabs(["machine", "processes"]);
    act(() => (document.activeElement as HTMLElement | null)?.blur());
    const press = modW(window);
    expect(panel().surfaces.map(surface => surface.id)).toEqual(ids);
    // Left to the browser, which in a tab closes the tab: the page never closes anything it does not hold.
    expect(press).toBe(true);
  });

  it("closes the tab when the desktop shell hands it on from a browser tab's page", async () => {
    let chord: ((c: { key: string; code: string; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean; altKey: boolean }) => void) | undefined;
    window.wsp = { setTerminalFocus: () => {}, onShellChord: handler => ((chord = handler), () => (chord = undefined)) };
    const ids = await mountWithTabs(["machine", "processes"]);
    // The guest's element holds the page's focus while its page has the keys, and it stands in the guests' layer
    // beside the shell, over the tab's slot, not inside the panel.
    const layer = document.createElement("div");
    layer.dataset["browserGuest"] = "g1";
    const guest = document.createElement("div");
    guest.tabIndex = -1;
    layer.appendChild(guest);
    document.body.appendChild(layer);
    try {
      act(() => guest.focus());
      await waitFor(() => expect(chord).toBeDefined());
      act(() => chord!({ key: "w", code: "KeyW", metaKey: true, ctrlKey: false, shiftKey: false, altKey: false }));
      await waitFor(() => expect(panel().surfaces.map(surface => surface.id)).toEqual([ids[0]]));
    } finally {
      layer.remove();
    }
  });
});

describe("mod+w in a terminal in the right panel", () => {
  it("lets Command+w through to the app on macOS and keeps Control+w, the word erase, in the shell", () => {
    const root = document.createElement("div");
    root.dataset["previewPanelMode"] = "inline";
    const term = document.createElement("div");
    term.dataset["terminalOwner"] = "right-panel";
    const input = document.createElement("textarea");
    term.appendChild(input);
    root.appendChild(term);
    document.body.appendChild(root);
    input.focus();
    const w = (mods: { metaKey?: boolean; ctrlKey?: boolean }) => ({ key: "w", code: "KeyW", metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...mods });
    try {
      expect(isTerminalAppShortcut(w({ metaKey: true }), DEFAULT_RESOLVED_KEYBINDINGS, MAC)).toBe(true);
      expect(isTerminalAppShortcut(w({ ctrlKey: true }), DEFAULT_RESOLVED_KEYBINDINGS, LINUX)).toBe(false);
      expect(isTerminalAppShortcut(w({ ctrlKey: true }), DEFAULT_RESOLVED_KEYBINDINGS, MAC)).toBe(false);
    } finally {
      root.remove();
    }
  });
});
