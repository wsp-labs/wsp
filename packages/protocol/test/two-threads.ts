// SPDX-License-Identifier: AGPL-3.0-only
// Two threads in one project folder, as every folder project has run since its
// threads moved into the folder: one workspace record, thread A and thread B,
// one settled turn each, B the later. State keyed by the workspace is shared by
// both, so a test of anything a thread owns starts here and checks the other
// thread does not see it.
import type { SessionEvent, SessionView, WorkspaceView } from "../src/index.js";

export const FOLDER = "ws_folder";

export interface FolderThread {
  readonly workspaceId: string;
  readonly threadId: string;
  readonly sessionId: string;
  readonly turnId: string;
  /** What the person asked in its one turn, which titles its sidebar row. */
  readonly prompt: string;
  /** The agent's one reply, which only its own transcript holds. */
  readonly reply: string;
}

export const THREAD_A: FolderThread = { workspaceId: FOLDER, threadId: "thr_a", sessionId: "sess_a", turnId: "turn_a", prompt: "thread a asks", reply: "reply in thread a" };
export const THREAD_B: FolderThread = { workspaceId: FOLDER, threadId: "thr_b", sessionId: "sess_b", turnId: "turn_b", prompt: "thread b asks", reply: "reply in thread b" };

/** The folder's record. It remembers the session that started last, which is A's: the row the latest view draws is B's. */
export const folderWorkspace: WorkspaceView = {
  id: FOLDER,
  name: "lab",
  machineId: "m_lab",
  project: { id: "pr_lab", name: "lab", path: "/root/lab", computer: "default" },
  phase: "running",
  golden: "snap_g",
  createdAt: "2026-09-01T00:00:00Z",
  claudeSessionId: THREAD_A.sessionId,
};

function turn(t: FolderThread, at: number): SessionEvent[] {
  const ids = { workspaceId: t.workspaceId, threadId: t.threadId, sessionId: t.sessionId, turnId: t.turnId };
  return [
    { type: "session.start", ...ids, at, prompt: t.prompt },
    { type: "session.delta", ...ids, at: at + 300, kind: "text", text: t.reply },
    { type: "session.done", ...ids, at: at + 900, result: { status: "completed", durationMs: 900, costUsd: 0.001 } },
    { type: "session.end", ...ids, at: at + 950, exitCode: 0, sawResult: true },
  ];
}

function row(t: FolderThread, at: number): SessionView {
  return { id: `s_${t.threadId}`, workspaceId: t.workspaceId, harness: "claude", status: "completed", prompt: t.prompt, startedAt: at, endedAt: at + 950, readAt: at + 2_000, threadId: t.threadId, claudeSessionId: t.sessionId };
}

/** The folder's transcript and rows, A's turn at `t0` and B's a minute later, both read. Relative to now by default, so
 * the rows stay on the sidebar's idle shelf rather than walking into its archive as the calendar moves. */
export function twoThreads(t0 = Date.now() - 60 * 60_000): { history: SessionEvent[]; rows: [SessionView, SessionView]; t0: number } {
  return { history: [...turn(THREAD_A, t0), ...turn(THREAD_B, t0 + 60_000)], rows: [row(THREAD_A, t0), row(THREAD_B, t0 + 60_000)], t0 };
}
