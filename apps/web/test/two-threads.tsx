// SPDX-License-Identifier: AGPL-3.0-only
// The app over a host holding two threads in one project folder (the
// protocol's two-threads fixture), for any test of state a thread owns: the
// fake host records what the app sends, starts, stops and stamps read, and
// mountTwoThreads opens either thread from its sidebar row the way a person does.
import { act, fireEvent, render, waitFor, within } from "@testing-library/react";
import { expect } from "vitest";
import { harnessCatalog } from "@wsp/runtime";
import type { EventUnion, GoldenManifest, SessionEvent, SessionView } from "@wsp/protocol";
import { Shell } from "../src/App.js";
import type { Api, ProtocolEvent, StartSessionOptions } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { folderWorkspace, THREAD_A, THREAD_B, twoThreads, type FolderThread } from "../../../packages/protocol/test/two-threads.js";
import { whenAgentsAnswered } from "./agents.js";
import { caps } from "./caps.js";
import { noDaemonApi } from "./fake-daemon-api.js";

const manifest: GoldenManifest = {
  head: 1,
  versions: [{ version: 1, snapshotId: "snap_g", baseTemplate: "default", kind: "sandbox", setupSha: "x", createdAt: "t", smoke: { cmd: "true", exitCode: 0 } }],
};

export interface TwoThreadsOptions {
  /** The folder's rows in place of the fixture's two read ones. */
  rows?: (rows: [SessionView, SessionView], t0: number) => SessionView[];
  /** The history read, in place of answering the fixture's transcript. */
  history?: () => Promise<SessionEvent[]>;
  /** What an access pick on a thread answers; refused with the host's words by default. */
  setSessionAccess?: Api["setSessionAccess"];
  /** Roads the case answers itself, each given the fixture's own, such as a second project's workspace. */
  more?: (api: Api) => Partial<Api>;
}

export function twoThreadsApi(opts: TwoThreadsOptions = {}) {
  const fixture = twoThreads();
  let held = opts.rows === undefined ? [...fixture.rows] : opts.rows(fixture.rows, fixture.t0);
  const started: StartSessionOptions[] = [];
  const read: string[] = [];
  const interrupted: string[] = [];
  const listeners = new Set<(e: ProtocolEvent) => void>();
  const workspace = folderWorkspace;
  const api: Api = {
    listWorkspaces: async () => [workspace],
    getWorkspace: async () => workspace,
    createWorkspace: async () => workspace,
    watchStatuses: async () => [],
    nap: async () => workspace,
    wake: async () => workspace,
    capabilities: async () => caps(),
    preferences: async () => useStore.getState().preferences,
    setPreferences: async () => useStore.getState().preferences,
    startSession: async o => {
      started.push(o);
      return { id: "s_x", workspaceId: o.workspaceId, harness: "claude", status: "running" };
    },
    interruptSession: async id => {
      interrupted.push(id);
      return { outcome: "accepted" };
    },
    setSessionAccess: opts.setSessionAccess ?? (async () => Promise.reject(new Error("that thread is gone"))),
    portReach: async (_id, port) => ({ url: `https://m1-${port}.preview.example/?pt_token=e`, expiresAt: Date.now() + 3_600_000 }),
    daemon: noDaemonApi,
    sessionHistory: opts.history ?? (async () => [...fixture.history]),
    listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
    snapshotStorage: async () => null,
    rollbackSnapshot: async () => ({ lineage: { name: "default", head: null, versions: [] }, existingWorkspaces: "untouched" }),
    listSessions: async () => held,
    readThread: async threadId => {
      read.push(threadId);
      held = held.map(row => ((row.threadId ?? row.id) === threadId ? { ...row, readAt: Math.max(Date.now(), row.endedAt ?? 0) } : row));
    },
    // The runtime's own table, whose access modes the picker offers.
    listHarnesses: async () => [harnessCatalog("claude")!],
    subscribe: fn => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    getGolden: async () => manifest,
  };
  if (opts.more !== undefined) Object.assign(api, opts.more({ ...api }));
  const emit = (e: EventUnion) => act(() => { for (const fn of [...listeners]) fn(e); });
  return { api, started, read, interrupted, emit, t0: fixture.t0 };
}

/** The shell over the two threads, both rows in its sidebar, and a road to open either one. */
export async function mountTwoThreads(fixture = twoThreadsApi()) {
  useStore.getState().bind(fixture.api);
  await whenAgentsAnswered();
  const { container } = render(<Shell />);
  const sidebar = () => within(container.querySelector<HTMLElement>("[data-app-sidebar]")!);
  const center = () => within(container.querySelector<HTMLElement>("[data-shell-center]")!);
  const editor = () => center().getByTestId("composer-editor");
  const open = async (thread: FolderThread) => {
    fireEvent.click(sidebar().getByText(thread.prompt).closest<HTMLElement>("[data-sidebar-row]")!);
    await center().findByText(thread.reply);
    await waitFor(() => expect(editor().getAttribute("contenteditable")).toBe("true"));
  };
  await waitFor(() => expect(sidebar().getByText(THREAD_A.prompt)).toBeDefined());
  await waitFor(() => expect(sidebar().getByText(THREAD_B.prompt)).toBeDefined());
  return { ...fixture, container, sidebar, center, editor, openA: () => open(THREAD_A), openB: () => open(THREAD_B) };
}
