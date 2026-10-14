// SPDX-License-Identifier: AGPL-3.0-only
// The records the notices list is drawn over: two computers, three project folders, five threads, and six notices,
// each worded by the app's own words. Four came since the list was last opened: a thread's question, a computer's
// sign-in, a plan alert and the update. Two are older: a thread that stopped and a copy the clipboard refused.
import { askingLine, planAlertLine, setupNeedsYouLine, waitLine, type PlaceView, type SessionView, type WorkspaceView } from "@wsp/protocol";
import { HOST_NOTICE_WORDS } from "../../src/notices/hostNotices";
import { notCopied, type Notice } from "../../src/notices/store";
import { UPDATE_WORDS } from "../../src/shell/update";

const ago = (minutes: number): number => Date.now() - minutes * 60_000;

export const PLACES: PlaceView[] = [
  { id: "here", kind: "computer", name: "zingzy-mbp", label: "zingzy's MacBook Pro", default: true, present: true, mac: "macbook", takesForks: false } as PlaceView,
  { id: "p_hetzner", kind: "computer", name: "hetzner", default: false, present: true, takesForks: true } as PlaceView,
];

const record = (id: string, project: { id: string; name: string; path: string }, place?: string): WorkspaceView => ({
  id,
  name: project.name,
  machineId: place === undefined ? "local" : `m_${id}`,
  project: { ...project, computer: "default" },
  phase: "running",
  golden: "",
  createdAt: "2026-10-01T09:00:00Z",
  ...(place === undefined ? { kind: "local" as const } : { place }),
});

const WSP_MAC = { id: "pr_wsp", name: "wsp", path: "/Users/zingzy/wsp" };
const WSP_BOX = { id: "pr_wsp_box", name: "wsp", path: "/root/wsp" };
const LANDING = { id: "pr_landing", name: "spoo-landing", path: "/root/spoo-landing" };

export const WORKSPACES: WorkspaceView[] = [record("ws_mac", WSP_MAC), record("ws_box", WSP_BOX, "p_hetzner"), record("ws_landing", LANDING, "p_hetzner")];

const thread = (id: string, workspaceId: string, harness: string, prompt: string, status: SessionView["status"], started: number, ended?: number, asking?: string): SessionView =>
  ({ id, threadId: id, workspaceId, harness, prompt, status, startedBy: "person", startedAt: ago(started), ...(ended === undefined ? {} : { endedAt: ago(ended) }), ...(asking === undefined ? {} : { asking }) }) as SessionView;

const QUESTION = {
  toolName: "AskUserQuestion",
  input: JSON.stringify({ questions: [{ question: "Which branch should the fix land on?", header: "Branch", multiSelect: false, options: [{ label: "main" }, { label: "release/0.3.7" }] }] }),
};

const CODEX_SIGN_IN = { row: "signins/codex", label: "Codex", url: "https://auth.openai.com/device", code: "4F2K-9QJM", expiresAt: "2026-10-14T10:10:00.000Z", state: "waiting" as const };

export const SESSIONS: SessionView[] = [
  thread("t_fix", "ws_box", "claude", "Fix the relay's reconnect after a host restart", "running", 9, undefined, askingLine(QUESTION)),
  thread("t_coord", "ws_mac", "claude", "Coordinator: the launch map", "running", 50),
  thread("t_probe", "ws_landing", "codex", "Probe: send latency on the relay", "failed", 41, 34),
  thread("t_hero", "ws_landing", "claude", "Shrink the hero image under 200 KB", "running", 3),
  thread("t_docs", "ws_mac", "claude", "Rewrite the install page for the new CLI", "completed", 40, 33),
];

/** The thread open in the centre, which no notice is about: a notice for the thread on screen is never raised. */
export const OPEN_THREAD = { workspaceId: "ws_mac", threadId: "t_docs" };

/** A notice as the store keeps it, with what it is about: the thread (the field the build adds so a row names its
 * thread), the computer, or the page its act opens. */
export type ListNotice = Notice & { readonly thread?: { readonly workspaceId: string; readonly threadId: string }; readonly placeId?: string; readonly source?: "usage" | "update" };

const open = { word: HOST_NOTICE_WORDS.open, run: () => {} };

/** Newest first, as the store keeps them; the first four arrived since the list was last opened. */
export const NOTICES: ListNotice[] = [
  { id: "n6", kind: "waiting", text: askingLine(QUESTION), where: "wsp", at: ago(2), action: open, key: "ask:ws_box:t_fix:a1", thread: { workspaceId: "ws_box", threadId: "t_fix" } },
  { id: "n5", kind: "waiting", text: setupNeedsYouLine("hetzner", waitLine(CODEX_SIGN_IN)), where: "hetzner", at: ago(6), action: open, key: "sign-in:p_hetzner/signins/codex", placeId: "p_hetzner" },
  { id: "n4", kind: "note", text: planAlertLine("Claude Max", { kind: "low", window: "week", step: 90 }), at: ago(11), action: open, source: "usage" },
  { id: "n3", kind: "note", text: UPDATE_WORDS.ready("0.3.7"), at: ago(15), action: { word: UPDATE_WORDS.restart, run: () => {} }, source: "update" },
  { id: "n2", kind: "error", text: "Stopped before it replied: exit 1", where: "spoo-landing", at: ago(34), action: open, thread: { workspaceId: "ws_landing", threadId: "t_probe" } },
  { id: "n1", kind: "error", text: notCopied("Document is not focused."), at: ago(185) },
];

export const UNREAD = 4;
/** When the list was last opened: the four notices after it are the new ones. */
export const SEEN_BEFORE = ago(20);

/** The notice that lands while the page is up, for the toast standing under the bell. */
export const ARRIVING: ListNotice = { id: "n7", kind: "error", text: "Stopped before it replied: the machine ran out of memory", where: "spoo-landing", at: Date.now(), action: open, thread: { workspaceId: "ws_landing", threadId: "t_hero" } };
