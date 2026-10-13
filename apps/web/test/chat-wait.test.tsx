// SPDX-License-Identifier: AGPL-3.0-only
// A turn whose requests the host held while it was away reads, once it went on, one row saying how long it waited,
// read back from the transcript and pushed live alike; a sentence the host answered with instead reads as written.
import { act, render, screen, waitFor } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { EventUnion, SessionEvent, WorkspaceView } from "@wsp/protocol";
import { installFakeLayout } from "./fake-layout.js";
import { useStore } from "../src/protocol/store.js";
import type { Api, ProtocolEvent } from "../src/protocol/client.js";
import { ChatView } from "../src/components/chat/ChatView.js";
import { CHAT_T0, CHAT_WS } from "./fixtures/chat-stream.js";
import { caps } from "./caps.js";
import { noDaemonApi } from "./fake-daemon-api.js";

let restoreLayout: () => void = () => {};
beforeAll(() => { restoreLayout = installFakeLayout(); });
afterAll(() => restoreLayout());

const workspace: WorkspaceView = { id: CHAT_WS, name: "upload", machineId: "m1", project: { id: "pr_1", name: "lab", path: "/root", computer: "default" }, phase: "running", golden: "snap_g", createdAt: "2026-09-01T00:00:00Z" };
const MIN = 60_000;
const sc = { workspaceId: CHAT_WS, sessionId: "sess_w", turnId: "turn_w_1", threadId: "thr_w" };

function fixtureApi(history: SessionEvent[]) {
  const listeners = new Set<(e: ProtocolEvent) => void>();
  const api: Api = {
    portReach: async (_id, port) => ({ url: `https://m1-${port}.preview.example/?pt_token=e`, expiresAt: Date.now() + 3_600_000 }),
    daemon: noDaemonApi,
    sessionHistory: async () => history,
    listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
    snapshotStorage: async () => null,
    rollbackSnapshot: async () => ({ lineage: { name: "default", head: null, versions: [] }, existingWorkspaces: "untouched" }),
    listWorkspaces: async () => [workspace],
    getWorkspace: async () => workspace,
    createWorkspace: async () => workspace,
    nap: async () => workspace,
    wake: async () => workspace,
    capabilities: async () => caps(),
    listSessions: async () => [],
    watchStatuses: async () => [],
    subscribe: fn => { listeners.add(fn); return () => listeners.delete(fn); },
    getGolden: async () => undefined,
    startSession: async opts => ({ id: "s1", workspaceId: opts.workspaceId, harness: "claude", status: "running" }),
  };
  const emit = (e: EventUnion) => act(() => { for (const fn of [...listeners]) fn(e); });
  return { api, emit };
}

describe("a turn that waited for its computer", () => {
  it("reads one row with the minutes it waited, between the turn's own lines", async () => {
    const { api, emit } = fixtureApi([
      { type: "session.start", ...sc, at: CHAT_T0, model: "claude-sonnet-4-5", prompt: "fix the upload test" },
      { type: "session.delta", ...sc, at: CHAT_T0 + 300, kind: "text", text: "Reading the test first." },
      { type: "thread.waited", ...sc, at: CHAT_T0 + 14 * MIN + 2_300, pos: 3, ms: 14 * MIN },
      { type: "session.delta", ...sc, at: CHAT_T0 + 14 * MIN + 2_400, kind: "text", text: "The upload test passes now." },
    ]);
    useStore.getState().bind(api);
    await waitFor(() => expect(useStore.getState().workspaces.length).toBe(1));
    render(<ChatView workspaceId={CHAT_WS} />);
    await screen.findByText("Waited 14 min for your computer");
    expect(screen.getAllByText("Waited 14 min for your computer")).toHaveLength(1);
    expect(screen.getByText("Reading the test first.")).toBeDefined();
    expect(screen.getByText("The upload test passes now.")).toBeDefined();
    expect(screen.queryByText(/Waited \d+ s for your computer/)).toBeNull();

    const said = "Codex's sign-in for cloud threads on acme's laptop ended. Sign in again in Settings, Agents.";
    emit({ type: "thread.waited", ...sc, at: CHAT_T0 + 15 * MIN, pos: 5, ms: 0, said, seq: 1 });
    await screen.findByText(said);
    expect(screen.getAllByText(/^Waited /)).toHaveLength(1);
  });
});
