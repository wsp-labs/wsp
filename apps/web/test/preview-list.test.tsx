// SPDX-License-Identifier: AGPL-3.0-only
// The Browser pane's list of recent addresses and local servers: no helper sentence under it, sentence case
// headings, and no field of its own. The address bar is the one field: while the list shows, what is typed there
// filters it by port, address, process and page title, with a quiet line when nothing matches, and Enter still
// opens the address.
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { DAEMON_VERSION, NAMED_WATCH_DAEMON_VERSION, placeBehindLine, type PlaceView, type WorkspaceView } from "@wsp/protocol";
import { useStore } from "../src/protocol/store.js";
import type { PreviewableServer } from "../src/adapt/view-model.js";
import { recentsKey, type BrowserHistoryEntry } from "../src/browser/recents.js";
import { currentPlace, useBrowserTabs } from "../src/browser/tabs.js";
import { BrowserSurface } from "../src/components/preview/BrowserSurface.js";
import { PREVIEW_EMPTY, PREVIEW_NO_MATCH, PreviewEmptyState } from "../src/components/preview/PreviewEmptyState.js";

afterEach(() => {
  cleanup();
  window.localStorage.clear();
  useStore.setState({ places: [], workspaces: [] });
});

const server = (port: number, processName: string): PreviewableServer => ({
  host: "localhost",
  port,
  url: `http://localhost:${port}/`,
  processName,
  pid: port,
  terminal: null,
  source: "scanner",
  requestedUrl: `http://localhost:${port}/`,
});

const recent = (url: string, title?: string): BrowserHistoryEntry => ({ url, lastVisitedAt: Date.now() - 60_000, ...(title === undefined ? {} : { title }) });

const draw = (servers: PreviewableServer[], recents: BrowserHistoryEntry[] = [], query = "") =>
  render(<PreviewEmptyState servers={servers} recentEntries={recents} query={query} onRemoveRecent={() => {}} onOpenUrl={() => {}} />);

const ports = (): string[] => [...document.querySelectorAll("[data-server-port], [data-recent-url]")].map(el => el.getAttribute("data-server-port") ?? el.getAttribute("data-recent-url") ?? "");

describe("the Browser pane's list", () => {
  it("says nothing under the servers: the heading and the rows are enough", () => {
    draw([server(3000, "node"), server(8001, "python3")]);
    expect(document.body.textContent).not.toMatch(/Select a live local server/);
    const headings = [...document.querySelectorAll("h2")];
    expect(headings.map(h => h.textContent)).toEqual(["Local servers"]);
    for (const h of headings) expect(h.className).not.toMatch(/uppercase|font-mono|tracking/);
  });

  it("draws no field of its own, however many rows it holds", () => {
    draw([server(3000, "node"), server(8001, "python3"), server(5173, "vite"), server(4000, "ruby"), server(9229, "deno")], [recent("http://localhost:6006/")]);
    expect(screen.queryAllByRole("textbox")).toEqual([]);
  });

  it("filters by the query it is handed, matching port, address, process and page title, at any length", () => {
    const servers = [server(3000, "node"), server(8001, "python3"), server(5173, "vite")];
    const recents = [recent("http://localhost:6006/?path=/story", "Storybook"), recent("http://localhost:8787/")];
    const at = (query: string): string[] => {
      cleanup();
      draw(servers, recents, query);
      return ports();
    };
    expect(at("8001")).toEqual(["8001"]);
    expect(at("VITE")).toEqual(["5173"]);
    expect(at("storybook")).toEqual(["http://localhost:6006/?path=/story"]);
    expect(at("localhost:8787")).toEqual(["http://localhost:8787/"]);
    expect(at("nothing listens here")).toEqual([]);
    expect(screen.getByText(PREVIEW_NO_MATCH)).toBeDefined();
    expect(at("")).toHaveLength(5);
  });
});

describe("the address bar over the list", () => {
  const WS = "ws_list";
  const mount = () => {
    window.localStorage.setItem(recentsKey(WS), JSON.stringify([recent("http://localhost:6006/?path=/story", "Storybook"), recent("http://localhost:8787/")]));
    useBrowserTabs.setState({ byWorkspaceId: {} });
    const tabId = useBrowserTabs.getState().createTab(WS, null);
    render(<BrowserSurface workspaceId={WS} surface={{ id: `browser:${tabId}`, kind: "preview", resourceId: tabId }} />);
    return tabId;
  };
  const bar = (): HTMLInputElement => document.querySelector<HTMLInputElement>("[data-preview-url-input]")!;

  it("filters the list as the person types, keeps what they typed once they look away, and opens the address on Enter", async () => {
    const tabId = mount();
    expect(screen.getAllByRole("textbox")).toEqual([bar()]);
    act(() => bar().focus());
    fireEvent.change(bar(), { target: { value: "storybook" } });
    expect(ports()).toEqual(["http://localhost:6006/?path=/story"]);
    act(() => bar().blur());
    expect(bar().value).toBe("storybook");
    expect(ports()).toEqual(["http://localhost:6006/?path=/story"]);
    act(() => bar().focus());
    fireEvent.change(bar(), { target: { value: "localhost:3000" } });
    fireEvent.keyDown(bar(), { key: "Enter" });
    expect(currentPlace(useBrowserTabs.getState().byWorkspaceId[WS]?.[tabId] ?? null)).toEqual({ port: 3000, path: "/" });
  });
});

describe("the Browser pane of a thread on a computer whose daemon is too old to watch its ports", () => {
  const WS = "ws_box";
  const folder: WorkspaceView = { id: WS, project: { id: "pr_lab", name: "lab", path: "/root/lab", computer: "p_2" }, name: WS, kind: "place", place: "p_2", machineId: "p_2", phase: "running", golden: "", createdAt: "2026-10-13T00:00:00.000Z" };
  const hetzner = (daemonVersion?: number): PlaceView => ({ id: "p_2", kind: "computer", name: "hetzner", default: false, present: true, takesForks: true, buildsImages: false, ...(daemonVersion === undefined ? {} : { daemonVersion }) });
  const said = (daemonVersion?: number, recents: BrowserHistoryEntry[] = []): string => {
    cleanup();
    window.localStorage.setItem(recentsKey(WS), JSON.stringify(recents));
    useStore.setState({ places: [hetzner(daemonVersion)], workspaces: [folder] });
    useBrowserTabs.setState({ byWorkspaceId: {} });
    const tabId = useBrowserTabs.getState().createTab(WS, null);
    render(<BrowserSurface workspaceId={WS} surface={{ id: `browser:${tabId}`, kind: "preview", resourceId: tabId }} />);
    return document.querySelector("[data-browser-surface]")!.textContent ?? "";
  };

  it("says that computer's update line in place of the empty sentence, and the empty sentence once it can watch", () => {
    const old = NAMED_WATCH_DAEMON_VERSION - 1;
    expect(said(old)).toContain(placeBehindLine("hetzner", `daemon ${old}, host ${DAEMON_VERSION}`));
    expect(said(old)).not.toContain(PREVIEW_EMPTY);
    expect(said(undefined)).toContain("hetzner is behind: daemon version unknown");
    expect(said(NAMED_WATCH_DAEMON_VERSION)).toContain(PREVIEW_EMPTY);
    expect(said(NAMED_WATCH_DAEMON_VERSION)).not.toContain("is behind");
  });

  it("says the update line above the list once the folder has a recent address, and nothing of it once it can watch", () => {
    const old = NAMED_WATCH_DAEMON_VERSION - 1;
    const opened = [recent("http://localhost:8090/")];
    expect(said(old, opened)).toContain(placeBehindLine("hetzner", `daemon ${old}, host ${DAEMON_VERSION}`));
    expect(ports()).toEqual(["http://localhost:8090/"]);
    expect(said(NAMED_WATCH_DAEMON_VERSION, opened)).not.toContain("is behind");
    expect(ports()).toEqual(["http://localhost:8090/"]);
  });
});
