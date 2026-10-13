// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's start as a window sees it, in the order the host tells it:
// the record under its lead, its thread held with the start's steps, then the
// stages of its machine being made. From the record's first frame the sidebar
// holds it under its lead and its page is the start's line, never the setup
// card a create drawn at the sidebar's root shows.
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import type { EventUnion, SessionView, SubRow, WorkspaceView } from "@wsp/protocol";
import { Shell } from "../src/App.js";
import type { Api } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { caps } from "./caps.js";
import { noDaemonApi } from "./fake-daemon-api.js";
import { installFakeLayout } from "./fake-layout.js";
import { clearNotices } from "./notice-text.js";
import { TRANSCRIPT_LOADING } from "../src/transcript-words.js";

const NOW = Date.now();
const PROJECT = { id: "pr_web", name: "web", path: "/Users/dev/web", computer: "here" };
const LEAD: WorkspaceView = { id: "ws_lead", name: "web", machineId: "local", kind: "local", project: PROJECT, phase: "running", golden: "", createdAt: new Date(NOW - 3_600_000).toISOString() };
const KID: WorkspaceView = {
  id: "ws_kid",
  name: "k6 full suite on main 2",
  machineId: "",
  kind: "cloud",
  project: PROJECT,
  phase: "queued",
  golden: "snap_g",
  createdAt: new Date(NOW).toISOString(),
  parentThreadId: "th_lead",
  rootThreadId: "th_lead",
  queue: { since: new Date(NOW).toISOString(), why: "slot" },
};
const leadRow: SessionView = { id: "s_lead", workspaceId: LEAD.id, harness: "claude", status: "running", threadId: "th_lead", prompt: "Launch the k6 load tests", startedAt: NOW - 600_000 };
const sub: SubRow = {
  state: "starting",
  on: "boat",
  machine: KID.name,
  size: { cpu: 8, memMb: 16_384, name: "large" },
  rateUsdPerHour: 0.072,
  branch: "wsp/k6-full-suite-on-main-2",
  spawn: { startedAt: NOW - 3_000, branch: { branch: "k6/main-run", sha: "16ead78a1b2c3d4e5f60718293a4b5c6d7e8f901", ms: 3_000 }, machine: { since: NOW, states: [] }, log: [] },
};
const kidRow: SessionView = {
  id: "s_kid",
  workspaceId: KID.id,
  harness: "claude",
  status: "running",
  threadId: "th_kid",
  parentThreadId: "th_lead",
  rootThreadId: "th_lead",
  startedBy: "agent",
  prompt: "This thread runs on a machine of its own on Boat.\n\nThe task:\n\nRun the k6 full suite on main.",
  startedAt: NOW,
  sub,
};

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

/** Every frame from the call on, read for the sub-thread drawn anywhere but under its lead: a root tile, the setup
 * card, a row being made, or its page with no start line once its transcript is read. */
function watchFrames(): { seen: string[]; look: () => void; stop: () => void } {
  const seen: string[] = [];
  const look = (): void => {
    const kid = tileOf("th_kid") ?? tileOf(KID.id);
    const lead = tileOf("th_lead");
    if (kid !== null && (Number(kid.dataset["depth"]) < 1 || lead === null || !lead.closest("li")!.contains(kid))) seen.push("a root tile");
    if (document.querySelector("[data-testid=workspace-creation]") !== null) seen.push("the setup card");
    const page = document.querySelector("[data-chat-view]");
    if (useStore.getState().selectedThreadId === "th_kid" && page !== null && page.querySelector("[data-spawn-line]") === null && !page.textContent!.includes(TRANSCRIPT_LOADING)) seen.push("a page with no start line");
    if (useStore.getState().creations.length > 0) seen.push("a creation row");
  };
  const watch = new MutationObserver(look);
  watch.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
  return { seen, look, stop: () => (look(), watch.disconnect()) };
}

describe("a sub-thread's start", () => {
  it("stands under its lead and shows the start's line on its page from the record's first frame, through its machine's stages", async () => {
    const sessions = [leadRow];
    const workspaces = [LEAD];
    useStore.getState().bind(fakeApi(workspaces, sessions));
    render(<Shell />);
    await waitFor(() => expect(tileOf("th_lead")).not.toBeNull());

    const { seen, look, stop } = watchFrames();

    workspaces.push(KID);
    apply({ type: "workspace.created", workspace: KID });
    look();
    expect(tileOf(KID.id) ?? tileOf("th_kid")).not.toBeNull();

    sessions.push(kidRow);
    apply({ type: "session.held", workspaceId: KID.id, threadId: "th_kid" });
    apply({ type: "session.row", workspaceId: KID.id, threadId: "th_kid", id: kidRow.id, row: kidRow });
    await waitFor(() => expect(tileOf("th_kid")).not.toBeNull());
    act(() => useStore.getState().select(KID.id, "th_kid"));
    // The page is the thread's from its first frame: its transcript's read, then the start's line, live.
    expect(document.querySelector("[data-chat-view]")!.textContent).toContain(TRANSCRIPT_LOADING);
    await waitFor(() => expect(document.querySelector("[data-spawn-line]")?.getAttribute("data-spawn-line")).toBe("live"));
    expect(document.querySelector("[data-k=spawn-line]")!.textContent).toMatch(/^Starting a machine on Boat/);

    const asked = { threadId: "th_lead", rootThreadId: "th_lead" };
    apply({ type: "workspace.creating", workspaceId: KID.id, name: KID.name, stage: "fork-requested", message: `starting ${KID.name} on Boat`, elapsedMs: 0, askedBy: asked });
    apply({ type: "workspace.creating", workspaceId: KID.id, name: KID.name, stage: "fork-requested", message: "Boat's machine bx_khebx2kb is provisioned", elapsedMs: 2_000, askedBy: asked });
    stop();

    expect(seen).toEqual([]);
    expect(document.querySelector("[data-spawn-line]")?.getAttribute("data-spawn-line")).toBe("live");
    expect(Number(tileOf("th_kid")!.dataset["depth"])).toBe(1);
  });

  it("stands under its lead in a window opened while its machine is made, which the host tells no stage of it", async () => {
    // The record and its row are what the host holds mid-start; its subscribe replays no stage of a sub-thread.
    const making: WorkspaceView = { ...KID, phase: "queued" };
    delete making.queue;
    const api = fakeApi([LEAD, making], [leadRow, kidRow]);
    const listed = api.listWorkspaces;
    api.listWorkspaces = async () => (await new Promise(r => setTimeout(r, 50)), listed());
    const { seen, stop } = watchFrames();
    useStore.getState().bind(api);
    render(<Shell />);
    await waitFor(() => expect(tileOf("th_kid")).not.toBeNull());
    stop();
    expect(seen).toEqual([]);
    expect(Number(tileOf("th_kid")!.dataset["depth"])).toBe(1);
  });

  it("keeps a first start that failed under its lead, its page saying why with Try again", async () => {
    const said = "Boat gave two machines in a row that could not run commands (bx_3, bx_4); Boat is restoring badly right now. Start it again in a few minutes.";
    const released: WorkspaceView = { ...KID, phase: "released" };
    delete released.queue;
    const failed: SessionView = { ...kidRow, status: "failed", failure: said, endedAt: NOW + 40_000, sub: { ...sub, state: "released", spawn: { ...sub.spawn!, asked: kidRow.prompt!, failed: { step: "machine", said } } } };
    useStore.getState().bind(fakeApi([LEAD, released], [leadRow, failed]));
    const { seen, stop } = watchFrames();
    render(<Shell />);
    await waitFor(() => expect(tileOf("th_kid")).not.toBeNull());
    act(() => useStore.getState().select(KID.id, "th_kid"));
    await waitFor(() => expect(document.querySelector("[data-spawn-line]")?.getAttribute("data-spawn-line")).toBe("failed"));
    stop();
    expect(seen).toEqual([]);
    expect(Number(tileOf("th_kid")!.dataset["depth"])).toBe(1);
    expect(document.querySelector("[data-k=spawn-line]")!.textContent).toBe("Could not start a machine on Boat");
    expect(document.querySelector('[data-spawn-step="machine"]')!.textContent).toContain("Boat gave two machines in a row");
    expect(document.querySelector("[data-k=spawn-try-again]")).not.toBeNull();
  });
});
