// SPDX-License-Identifier: AGPL-3.0-only
// Two threads in one project folder share one workspace record, and nothing a
// thread holds outside its transcript crosses to the other: what is typed and
// attached, the access refusal on its picker, the row a send resumes, the
// running turn a Stop ends, and the read stamp a view puts on the host. A new
// thread's page holds none of either.
import { act, cleanup, fireEvent, render, waitFor, within } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_PREFERENCES, QUESTION_TOOL, questionOptions, type SessionEvent, type SessionView, type WorkspaceView } from "@wsp/protocol";
import { Shell } from "../src/App.js";
import { useStore } from "../src/protocol/store.js";
import { insertIntoComposer } from "../src/components/chat/composerInsert.js";
import { sendToThread } from "../src/diffs/sendToThread.js";
import { useComposerDraftStore } from "../src/components/chat/composerDraftStore.js";
import { useComposerFilesStore } from "../src/components/chat/composerFiles.js";
import { FOLDER, THREAD_B, folderWorkspace, twoThreads } from "../../../packages/protocol/test/two-threads.js";
import { goToWorkspace } from "../src/shell/shellCommands.js";
import { clickIntoEditor, press, typeInto } from "./composer-harness.js";
import { installFakeLayout } from "./fake-layout.js";
import { clearNotices } from "./notice-text.js";
import { mountTwoThreads, twoThreadsApi } from "./two-threads.js";

let restoreLayout: () => void = () => {};
beforeAll(() => { restoreLayout = installFakeLayout(); });
afterAll(() => restoreLayout());
beforeEach(() => {
  window.localStorage.clear();
  useComposerDraftStore.setState({ drafts: {}, queues: {}, held: {}, unclaimed: {}, shown: {} });
  useComposerFilesStore.setState({ pending: {}, refused: {}, queued: {}, sent: {} });
  useStore.setState({ api: null, conn: "live", capabilities: null, workspaces: [], statuses: {}, costs: {}, spending: {}, selectedId: null, selectedThreadId: null, freshThread: false, creations: [], sessions: {}, ready: false, gaps: 0, preferences: DEFAULT_PREFERENCES });
  clearNotices();
});
afterEach(() => cleanup());

const pdfFile = (name: string) => new File([new TextEncoder().encode("%PDF-1.7 notes")], name, { type: "application/pdf" });
const composerFiles = (container: HTMLElement) => [...container.querySelectorAll<HTMLElement>("[data-composer-files] [data-chat-image], [data-composer-files] [data-chat-file]")];
const accessPicker = (container: HTMLElement) => container.querySelector<HTMLElement>('[data-composer-picker="access"]');
const settle = () => act(() => new Promise(resolve => setTimeout(resolve, 50)));

describe("a thread's composer is its own", () => {
  it("keeps what was typed in one thread out of the other's composer, and back where it was typed", async () => {
    const { openA, openB, editor } = await mountTwoThreads();
    await openA();
    await typeInto(editor(), "half a thought for thread a");
    await openB();
    expect(editor().textContent).not.toContain("half a thought for thread a");
    await openA();
    expect(editor().textContent).toContain("half a thought for thread a");
  });

  it("puts a quote or a review comment into the composer of the thread on screen, not into a sibling's", async () => {
    const { openA, openB, editor } = await mountTwoThreads();
    await openB();
    act(() => insertIntoComposer(FOLDER, "a quoted line"));
    act(() => sendToThread(FOLDER, "a review comment"));
    await waitFor(() => expect(editor().textContent).toContain("a review comment"));
    expect(editor().textContent).toContain("a quoted line");
    await openA();
    expect(editor().textContent).not.toContain("a quoted line");
    expect(editor().textContent).not.toContain("a review comment");
  });

  it("keeps a file attached in one thread out of the other's composer and its send", async () => {
    const { openA, openB, editor, container, started } = await mountTwoThreads();
    await openA();
    const input = container.querySelector<HTMLInputElement>("[data-composer-file-input]")!;
    act(() => void fireEvent.change(input, { target: { files: [pdfFile("for-thread-a.pdf")] } }));
    await waitFor(() => expect(composerFiles(container)).toHaveLength(1));
    await openB();
    expect(composerFiles(container)).toHaveLength(0);
    await typeInto(editor(), "go on in b");
    await press(editor(), "Enter");
    await waitFor(() => expect(started).toHaveLength(1));
    expect(started[0]).toMatchObject({ thread: THREAD_B.threadId });
    expect(started[0]!.attachments).toBeUndefined();
    await openA();
    expect(composerFiles(container)).toHaveLength(1);
  });

  it("drops an access refusal said on one thread's picker when the other thread is opened", async () => {
    const { openA, openB, container } = await mountTwoThreads();
    await openA();
    await waitFor(() => expect(accessPicker(container)).not.toBeNull());
    fireEvent.click(accessPicker(container)!);
    const edits = await waitFor(() => {
      const option = container.ownerDocument.querySelector<HTMLElement>('[data-composer-option="acceptEdits"]');
      expect(option).not.toBeNull();
      return option!;
    });
    fireEvent.click(edits);
    await waitFor(() => expect(accessPicker(container)?.getAttribute("data-access-refused")).toContain("that thread is gone"));
    await openB();
    expect(accessPicker(container)?.getAttribute("data-access-refused") ?? null).toBeNull();
  });

  it("hands a draft an earlier build kept for the whole folder to the thread on screen after the update, and to no other", async () => {
    window.localStorage.setItem("wsp:composer-drafts:v1", JSON.stringify({ state: { drafts: { [FOLDER]: { prompt: "typed before the update", cursor: 22 } }, queues: {} }, version: 1 }));
    await act(() => useComposerDraftStore.persist.rehydrate());
    const { openA, openB, editor, center } = await mountTwoThreads();
    // The app opens on the folder's latest thread, which is B.
    await center().findByText(THREAD_B.reply);
    await waitFor(() => expect(editor().textContent).toContain("typed before the update"));
    await openA();
    expect(editor().textContent).not.toContain("typed before the update");
    await openB();
    expect(editor().textContent).toContain("typed before the update");
  });
});

describe("words from outside the composer while something stands in its place", () => {
  it("go to the thread whose agent asks a question, and are in its composer once the person writes instead", async () => {
    const ask = { workspaceId: FOLDER, threadId: THREAD_B.threadId, sessionId: "sess_b2", turnId: "turn_b2" };
    const input = JSON.stringify({ questions: [{ question: "Which port?", header: "Port", multiSelect: false, options: [{ label: "3000", description: "" }, { label: "8080", description: "" }] }] });
    const fixture = twoThreads();
    const asking: SessionEvent[] = [
      ...fixture.history,
      { type: "session.start", ...ask, at: fixture.t0 + 120_000, prompt: "thread b goes on" },
      { type: "session.permission", ...ask, at: fixture.t0 + 120_500, askId: "ask_1", toolName: QUESTION_TOOL, toolUseId: "toolu_ask_1", input, options: questionOptions(QUESTION_TOOL, input) },
    ];
    const running = (b: SessionView, t0: number): SessionView => ({ ...b, id: "s_b2", status: "running", prompt: "thread b goes on", startedAt: t0 + 120_000, endedAt: undefined, claudeSessionId: "sess_b2" });
    const { sidebar, container, editor } = await mountTwoThreads(twoThreadsApi({ history: async () => asking, rows: ([a, b], t0) => [a, b, running(b, t0)] }));
    fireEvent.click(sidebar().getByText(THREAD_B.prompt).closest<HTMLElement>("[data-sidebar-row]")!);
    await waitFor(() => expect(container.querySelector("[data-prompt-root]")).not.toBeNull());
    act(() => sendToThread(FOLDER, "a review comment"));
    fireEvent.click(container.querySelector<HTMLElement>("[data-prompt-write]")!);
    await waitFor(() => expect(editor().textContent).toContain("a review comment"));
    expect(useComposerDraftStore.getState().drafts[FOLDER]?.prompt ?? "").toBe("");
  });
});

describe("a chord to another project's thread", () => {
  it("puts the caret in that thread's composer when the last one held it", async () => {
    const OTHER = "ws_other";
    const other: WorkspaceView = { ...folderWorkspace, id: OTHER, name: "acme", project: { id: "pr_acme", name: "acme", path: "/root/acme", computer: "default" } };
    const x = { workspaceId: OTHER, threadId: "thr_x", sessionId: "sess_x", turnId: "turn_x" };
    const t0 = Date.now() - 30 * 60_000;
    const xHistory: SessionEvent[] = [
      { type: "session.start", ...x, at: t0, prompt: "thread x asks" },
      { type: "session.delta", ...x, at: t0 + 300, kind: "text", text: "reply in thread x" },
      { type: "session.done", ...x, at: t0 + 900, result: { status: "completed", durationMs: 900, costUsd: 0.001 } },
      { type: "session.end", ...x, at: t0 + 950, exitCode: 0, sawResult: true },
    ];
    const xRow: SessionView = { id: "s_x", workspaceId: OTHER, harness: "claude", status: "completed", prompt: "thread x asks", startedAt: t0, endedAt: t0 + 950, readAt: t0 + 2_000, threadId: "thr_x", claudeSessionId: "sess_x" };
    const { openA, center, editor } = await mountTwoThreads(
      twoThreadsApi({
        more: own => ({
          listWorkspaces: async () => [folderWorkspace, other],
          getWorkspace: async id => (id === OTHER ? other : folderWorkspace),
          sessionHistory: async id => (id === OTHER ? xHistory : own.sessionHistory(id)),
          listSessions: async id => (id === OTHER ? [xRow] : own.listSessions(id)),
        }),
      }),
    );
    await openA();
    clickIntoEditor(editor());
    await act(async () => await new Promise(resolve => setTimeout(resolve, 20)));
    act(() => goToWorkspace(OTHER, "thr_x"));
    await center().findByText("reply in thread x");
    await waitFor(() => expect(document.activeElement).toBe(editor()));
  });
});

describe("a new thread's page holds no sibling's turn", () => {
  it("leaves a sibling's unread finished turn unread", async () => {
    const { openA, read, center } = await mountTwoThreads(twoThreadsApi({ rows: ([a, b], t0) => [a, { ...b, readAt: t0 + 30_000 }] }));
    await openA();
    read.length = 0;
    act(() => useStore.getState().newThread(FOLDER));
    await waitFor(() => expect(center().queryByText("reply in thread a")).toBeNull());
    await settle();
    expect(read).not.toContain(THREAD_B.threadId);
  });

  it("offers no Stop for a sibling's turn held back by its computer's cap, and a press of Escape ends nothing", async () => {
    const capped = { placeId: "p_box", place: "box", running: 2, atOnce: 2 };
    const held = (b: SessionView, t0: number): SessionView => ({ id: "s_b2", workspaceId: FOLDER, harness: "claude", status: "running", prompt: "thread b goes on", startedAt: t0 + 120_000, threadId: b.threadId!, capped });
    const { openA, interrupted, center, editor } = await mountTwoThreads(twoThreadsApi({ rows: ([a, b], t0) => [a, b, held(b, t0)] }));
    await openA();
    act(() => useStore.getState().newThread(FOLDER));
    await waitFor(() => expect(center().queryByText("reply in thread a")).toBeNull());
    await waitFor(() => expect(editor().getAttribute("contenteditable")).toBe("true"));
    expect(center().queryByRole("button", { name: "Stop generation" })).toBeNull();
    await press(editor(), "Escape");
    expect(interrupted).toEqual([]);
  });

  it("shows no usage limit a sibling's turn stopped on", async () => {
    const { openA, center, container } = await mountTwoThreads(twoThreadsApi({ rows: ([a, b]) => [a, { ...b, status: "failed", limit: { resetsAt: Date.now() + 3_600_000 } }] }));
    await openA();
    act(() => useStore.getState().newThread(FOLDER));
    await waitFor(() => expect(center().queryByText("reply in thread a")).toBeNull());
    await settle();
    expect(container.querySelector(`[data-drawer-row="usage"]`)).toBeNull();
  });
});

describe("the view the app opens on", () => {
  it("shows the latest thread's own draft while its transcript loads, never the new thread page's", async () => {
    useComposerDraftStore.setState({ drafts: { [FOLDER]: { prompt: "left on the new thread page", cursor: 27 }, [THREAD_B.threadId]: { prompt: "b's own words", cursor: 13 } } });
    let answer: (events: SessionEvent[]) => void = () => {};
    const { editor, center } = await mountTwoThreads(twoThreadsApi({ history: () => new Promise(resolve => (answer = resolve)) }));
    await waitFor(() => expect(editor().textContent).toContain("b's own words"));
    expect(editor().textContent).not.toContain("left on the new thread page");
    answer(twoThreads().history);
    await center().findByText(THREAD_B.reply);
    expect(editor().textContent).toContain("b's own words");
  });

  it("leaves the new thread page's words on that page when the folder's rows come in late", async () => {
    useComposerDraftStore.setState({ drafts: { [FOLDER]: { prompt: "an idea for a new thread", cursor: 24 } } });
    let refuse = true;
    useStore.getState().bind(twoThreadsApi({ more: own => ({ listSessions: async id => (refuse ? Promise.reject(new Error("rows offline")) : own.listSessions(id)) }) }).api);
    const center = within(render(<Shell />).container.querySelector<HTMLElement>("[data-shell-center]")!);
    await center.findByText(THREAD_B.reply);
    refuse = false;
    await act(() => useStore.getState().refresh());
    expect(center.getByTestId("composer-editor").textContent).not.toContain("an idea for a new thread");
    act(() => useStore.getState().newThread(FOLDER));
    await waitFor(() => expect(center.getByTestId("composer-editor").textContent).toContain("an idea for a new thread"));
  });
});

describe("a send resumes the thread its view shows", () => {
  it("resumes the thread the latest view draws, never the session the workspace last started, when the view's history did not come", async () => {
    const { started, center, editor } = await mountTwoThreads(twoThreadsApi({ history: async () => Promise.reject(new Error("history store offline")) }));
    await center().findByText(/history unavailable/i);
    await waitFor(() => expect(editor().getAttribute("contenteditable")).toBe("true"));
    await typeInto(editor(), "carry on");
    await press(editor(), "Enter");
    await waitFor(() => expect(started).toHaveLength(1));
    expect(started[0]!.thread).toBe(THREAD_B.threadId);
  });
});
