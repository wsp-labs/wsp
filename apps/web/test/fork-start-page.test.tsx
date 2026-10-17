// SPDX-License-Identifier: AGPL-3.0-only
// A fork a thread asked for, as a window sees it in the order the host tells
// it: the record under the thread that asked while its machine is made, the
// stages of that machine, the record again once it stands, then the thread the
// lead opens on it. From the record's first frame the sidebar holds it under
// its lead, never as a create at the sidebar's root.
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import type { EventUnion, SessionView, WorkspaceView } from "@wsp/protocol";
import { Shell } from "../src/App.js";
import type { Api } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { resolveActions } from "../src/actions/registry.js";
import { workspaceActions, workspaceTarget } from "../src/actions/workspaceActions.js";
import { caps } from "./caps.js";
import { noDaemonApi } from "./fake-daemon-api.js";
import { installFakeLayout } from "./fake-layout.js";
import { clearNotices } from "./notice-text.js";
import { TRANSCRIPT_LOADING } from "../src/transcript-words.js";
import { FAILED_START_WORDS } from "../src/shell/FailedStart.js";

const NOW = Date.now();
const PROJECT = { id: "pr_web", name: "web", path: "/Users/dev/web", computer: "here" };
const CLOUD = { id: "pr_web_boat", name: "web", path: "/root/web", computer: "boat" };
const LEAD: WorkspaceView = { id: "ws_lead", name: "web", machineId: "local", kind: "local", project: PROJECT, phase: "running", golden: "", createdAt: new Date(NOW - 3_600_000).toISOString() };
const MAKING: WorkspaceView = {
  id: "ws_fork",
  name: "pricing table",
  machineId: "",
  kind: "cloud",
  project: CLOUD,
  phase: "queued",
  golden: "",
  createdAt: new Date(NOW).toISOString(),
  parentThreadId: "th_lead",
  rootThreadId: "th_lead",
  parentWorkspaceId: LEAD.id,
};
const WHY = "Boat refused the machine: 3 of 3 machines at once are in use (lint, k6 suite, docs).";
const FAILED: WorkspaceView = { ...MAKING, phase: "gone", gone: WHY };
const MADE: WorkspaceView = { ...MAKING, machineId: "bx_khebx2kb", phase: "running", golden: "wsp-host-default-v1" };
const leadRow: SessionView = { id: "s_lead", workspaceId: LEAD.id, harness: "claude", status: "running", threadId: "th_lead", prompt: "Fork the pricing table work", startedAt: NOW - 600_000 };
const forkRow: SessionView = { id: "s_fork", workspaceId: MADE.id, harness: "claude", status: "running", threadId: "th_fork", parentThreadId: "th_lead", rootThreadId: "th_lead", startedBy: "agent", prompt: "Build the pricing table.", startedAt: NOW + 30_000 };

function fakeApi(workspaces: WorkspaceView[], sessions: SessionView[]): Api {
  return {
    listWorkspaces: async () => workspaces,
    getWorkspace: async id => workspaces.find(w => w.id === id)!,
    createWorkspace: async () => workspaces[0]!,
    watchStatuses: async () => [],
    nap: async id => workspaces.find(w => w.id === id)!,
    wake: async id => workspaces.find(w => w.id === id)!,
    capabilities: async () => caps(),
    startSession: async o => ({ id: "s1", workspaceId: o.workspaceId, harness: "claude", status: "running" }),
    portReach: async (_id, port) => ({ url: `https://m1-${port}.preview.example/?pt_token=e`, expiresAt: NOW + 3_600_000 }),
    daemon: noDaemonApi,
    sessionHistory: async () => [],
    listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
    snapshotStorage: async () => null,
    rollbackSnapshot: async () => ({ lineage: { name: "default", head: null, versions: [] }, existingWorkspaces: "untouched" }),
    listSessions: async id => (id === undefined ? sessions : sessions.filter(s => s.workspaceId === id)),
    subscribe: () => () => {},
    getGolden: async () => ({ head: 1, versions: [{ version: 1, snapshotId: "snap_g", baseTemplate: "t", setupSha: "s", createdAt: "c", smoke: { cmd: "true", exitCode: 0 } }] }),
  };
}

let restoreLayout: () => void = () => {};
beforeAll(() => {
  restoreLayout = installFakeLayout();
});
afterAll(() => restoreLayout());
beforeEach(() => {
  window.localStorage.clear();
  useStore.setState({ api: null, conn: "live", capabilities: null, workspaces: [], statuses: {}, costs: {}, spending: {}, selectedId: null, selectedThreadId: null, creations: [], sessions: {}, launches: {}, ready: false });
  clearNotices();
});
afterEach(cleanup);

/** A thread's tile, or a workspace's before its thread is listed. */
const tileOf = (id: string): HTMLElement | null => document.querySelector<HTMLElement>(`[data-row-id='thread:${id}'], [data-row-id='ws:${id}']`);
const apply = (e: EventUnion): void => act(() => useStore.getState().applyEvent(e));

/** Every frame from the call on, read for the fork drawn anywhere but under its lead: a root tile, the setup card of
 * a create, or a row being made. */
function watchFrames(): { seen: string[]; look: () => void; stop: () => void } {
  const seen: string[] = [];
  const look = (): void => {
    const fork = tileOf("th_fork") ?? tileOf(MAKING.id);
    const lead = tileOf("th_lead");
    if (fork !== null && (Number(fork.dataset["depth"]) < 1 || lead === null || !lead.closest("li")!.contains(fork))) seen.push("a root tile");
    if (document.querySelector("[data-testid=workspace-creation]") !== null) seen.push("the setup card");
    if (useStore.getState().creations.length > 0) seen.push("a creation row");
  };
  const watch = new MutationObserver(look);
  watch.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
  return { seen, look, stop: () => (look(), watch.disconnect()) };
}

describe("a fork a thread asked for", () => {
  it("stands under its lead from the record's first frame, through its machine's stages, to its thread", async () => {
    const sessions = [leadRow];
    const workspaces = [LEAD];
    useStore.getState().bind(fakeApi(workspaces, sessions));
    render(<Shell />);
    await waitFor(() => expect(tileOf("th_lead")).not.toBeNull());

    const { seen, look, stop } = watchFrames();
    const tell = (e: EventUnion): void => (apply(e), look());
    workspaces.push(MAKING);
    tell({ type: "workspace.created", workspace: MAKING });
    expect(tileOf(MAKING.id)).not.toBeNull();
    act(() => useStore.getState().select(MAKING.id, null));
    // Its page waits for the thread its lead opens on it.
    expect(document.querySelector("[data-chat-view]")!.textContent).toContain(TRANSCRIPT_LOADING);

    const asked = { threadId: "th_lead", rootThreadId: "th_lead" };
    tell({ type: "workspace.creating", workspaceId: MAKING.id, name: MAKING.name, stage: "fork-requested", message: `Starting ${MAKING.name} on Boat`, elapsedMs: 0, askedBy: asked });
    tell({ type: "workspace.creating", workspaceId: MAKING.id, name: MAKING.name, stage: "fork-requested", message: "Boat's machine bx_khebx2kb is provisioned", elapsedMs: 2_000, askedBy: asked });
    tell({ type: "workspace.creating", workspaceId: MAKING.id, name: MAKING.name, stage: "ready", message: "Ready.", elapsedMs: 9_000, askedBy: asked });
    workspaces.splice(1, 1, MADE);
    tell({ type: "workspace.created", workspace: MADE });
    sessions.push(forkRow);
    tell({ type: "session.row", workspaceId: MADE.id, threadId: "th_fork", id: forkRow.id, row: forkRow });
    await waitFor(() => expect(tileOf("th_fork")).not.toBeNull());
    stop();

    expect(seen).toEqual([]);
    expect(Number(tileOf("th_fork")!.dataset["depth"])).toBe(1);
  });

  it("stands under its lead in a window opened while its machine is made, which the host tells no stage of it", async () => {
    const api = fakeApi([LEAD, MAKING], [leadRow]);
    const listed = api.listWorkspaces;
    api.listWorkspaces = async () => (await new Promise(r => setTimeout(r, 50)), listed());
    const { seen, stop } = watchFrames();
    useStore.getState().bind(api);
    render(<Shell />);
    await waitFor(() => expect(tileOf(MAKING.id)).not.toBeNull());
    stop();
    expect(seen).toEqual([]);
    expect(Number(tileOf(MAKING.id)!.dataset["depth"])).toBe(1);
  });

  it("stays under its lead once its machine failed: its tile says so, its page says why and what to do next with no composer, and Delete is its one way out", async () => {
    const workspaces = [LEAD];
    const api = fakeApi(workspaces, [leadRow]);
    // The host holds no history for a fork that never had a machine, and refuses the read as it does any id it does not hold.
    const historyAsked: string[] = [];
    const history = api.sessionHistory;
    api.sessionHistory = async id => {
      historyAsked.push(id);
      if (id === FAILED.id) throw new Error(`no such workspace: ${id}`);
      return history(id);
    };
    useStore.getState().bind(api);
    render(<Shell />);
    await waitFor(() => expect(tileOf("th_lead")).not.toBeNull());
    const { seen, look, stop } = watchFrames();
    const tell = (e: EventUnion): void => (apply(e), look());
    workspaces.push(MAKING);
    tell({ type: "workspace.created", workspace: MAKING });
    const asked = { threadId: "th_lead", rootThreadId: "th_lead" };
    tell({ type: "workspace.creating", workspaceId: MAKING.id, name: MAKING.name, stage: "fork-requested", message: `Starting ${MAKING.name} on Boat`, elapsedMs: 0, askedBy: asked });
    workspaces.splice(1, 1, FAILED);
    tell({ type: "workspace.created", workspace: FAILED });
    tell({ type: "workspace.creating", workspaceId: MAKING.id, name: MAKING.name, stage: "failed", message: WHY, elapsedMs: 4_000, askedBy: asked });
    act(() => useStore.getState().select(FAILED.id, null));
    stop();

    expect(seen).toEqual([]);
    const tile = tileOf(FAILED.id)!;
    expect(Number(tile.dataset["depth"])).toBe(1);
    expect(tile.querySelector<HTMLElement>("[data-thread-status]")!.dataset["threadStatus"]).toBe("failed");
    // Long enough for a history read the page asked for to have come back refused and been drawn.
    await new Promise(r => setTimeout(r, 50));
    expect(document.body.textContent).not.toContain("History unavailable");
    expect(document.querySelector("[data-chat-composer-dock]")).toBeNull();
    const page = document.querySelector<HTMLElement>("[data-testid=failed-start]")!;
    expect(page.textContent).toContain(`Could not start ${FAILED.name}`);
    expect(page.textContent).toContain(WHY);
    expect(page.textContent).toContain(FAILED_START_WORDS.next);
    expect(document.querySelector("[data-chat-view]")).toBeNull();
    expect(historyAsked).not.toContain(FAILED.id);
    // What to do next is the lead's: the page opens it.
    act(() => (page.querySelector<HTMLElement>("[data-k=failed-start-lead]")!).click());
    expect(useStore.getState()).toMatchObject({ selectedId: LEAD.id, selectedThreadId: "th_lead" });
    const acts = resolveActions(workspaceActions, workspaceTarget(FAILED, null, []), { togglePhase: async () => {}, copyText: () => {}, rebuild: async () => {}, deleteWorkspace: async () => {}, forget: async () => {} } as never).map(a => a.id);
    expect(acts).toEqual(["delete"]);
  });
});
