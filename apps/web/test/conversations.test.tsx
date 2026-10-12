// SPDX-License-Identifier: AGPL-3.0-only
// A project's conversations kept outside wsp, in the app: the picker the palette's root and the project's menu open,
// searched over title, first prompt, branch and agent, scoped by agent, branch and project, its highlighted row's
// newest messages beside the list; Enter opens a row as a draft on the project's New thread page, its earlier messages
// shut in the Earlier fold a thread on it keeps, and ⇧Enter does the same on a copy; one open in another app asks
// first, offering a copy, and Try again where the agent lets go of a closed one.
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { cloneElement, type ReactElement, type ReactNode } from "react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { CONVERSATION_OPEN_KIND, CONVERSATION_WORDS, DEFAULT_PREFERENCES, conversationOpenFix, conversationOpenLine, earlierLine, refusal, type ConversationRead, type ConversationsAnswer, type HarnessCatalog, type OutsideConversation, type PlaceView, type ProjectView, type SessionEvent, type SessionView, type WorkspaceView } from "@wsp/protocol";
import { resolveActions } from "../src/actions/registry.js";
import { projectActions } from "../src/actions/projectActions.js";
import { openCommandPalette } from "../src/commandPaletteBus.js";
import { CommandPalette } from "../src/components/palette/CommandPalette.js";
import { buildPaletteItems } from "../src/components/palette/paletteItems.js";
import { SidebarProvider } from "../src/components/ui/sidebar.js";
import { deriveSession } from "../src/adapt/session.js";
import { EARLIER_FOLD, deriveMessagesTimelineRows, entryHiders } from "../src/adapt/timeline-rows.js";
import type { Api, StartSessionOptions } from "../src/protocol/client.js";
import { projectHomeKey, useStore } from "../src/protocol/store.js";
import { ConversationConfirm } from "../src/shell/ConversationConfirm.js";
import { ProjectHome } from "../src/shell/ProjectHome.js";
import { PREVIEWS_KEPT, openResumePicker, previewKey, useConversationsStore } from "../src/shell/conversations.js";
import { PREVIEW_REST_MS, ROWS_DRAWN, ResumePicker, pickerRows } from "../src/shell/ResumePicker.js";
import { useComposerDraftStore } from "../src/components/chat/composerDraftStore.js";
import { useComposerOptionsStore } from "../src/components/chat/composerOptionsStore.js";
import { useMultiPickStore } from "../src/components/chat/composerMultiPick.js";
import { composerEditor, press, typeInto } from "./composer-harness.js";
import { installFakeLayout } from "./fake-layout.js";
import { caps } from "./caps.js";
import { noDaemonApi } from "./fake-daemon-api.js";

vi.mock("../src/components/ui/tooltip.js", () => ({
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ render: element, children }: { render?: ReactElement<{ children?: ReactNode }>; children?: ReactNode }) =>
    element === undefined ? <>{children}</> : cloneElement(element, {}, children ?? element.props.children),
  TooltipPopup: () => null,
}));

const HERE: PlaceView = { id: "here", kind: "computer", name: "studio", label: "studio", default: true, present: true } as PlaceView;
const LAB: ProjectView = { id: "pr_lab", name: "lab", computer: "here", source: { kind: "folder", path: "/work/acme/lab" }, path: "/work/acme/lab", remote: "", defaultBranch: "main", memoryKey: "-x", memoryDir: "/x", createdAt: "t" };
const SPOO: ProjectView = { ...LAB, id: "pr_spoo", name: "spoo", source: { kind: "folder", path: "/work/acme/spoo" }, path: "/work/acme/spoo" };
const HOME = projectHomeKey(LAB.id);

const row = (over: Partial<OutsideConversation>): OutsideConversation => ({ agent: "claude", id: "7414323d-e71b-4957-8b56-eefdf6bfa350", title: "lab codewords", cwd: LAB.path, lastAt: Date.now() - 2 * 3_600_000, origin: "terminal", live: false, ...over });
const TERMINAL = row({ firstPrompt: "Remember the codeword ALPHA.", branch: "feat/x", bytes: 1_258_291 });
const OPEN = row({ id: "e615a6ab-a2f2-4b10-8fd6-0dc38bf99c28", title: "pricing page", live: true, lastAt: Date.now() - 60_000 });
const CODEX = row({ agent: "codex", id: "01a12813-cd12-7a12-9b13-e76892906ff0", title: "lab terminal thread", firstPrompt: "lab terminal thread", branch: "main", letsGo: "Codex lets go about a minute after its window closes.", lastAt: Date.now() - 5 * 60_000 });
const THREADED = row({ id: "11111111-2222-4333-8444-555555555555", title: "the old thread", origin: "wsp", thread: "thread-7f", lastAt: Date.now() - 86_400_000 });
const SPOO_ROW = row({ id: "22222222-3333-4444-8555-666666666666", title: "short links redirect twice", cwd: SPOO.path, lastAt: Date.now() - 3 * 60_000 });
const ANSWER: ConversationsAnswer = { rows: [OPEN, CODEX, TERMINAL, THREADED], held: [] };
const READ: ConversationRead = {
  cwd: "/home/dev/work/acme/lab-wt",
  title: "lab codewords",
  rows: [
    { who: "note", text: earlierLine(41, "Claude Code") },
    { who: "person", text: "Remember the codeword ALPHA." },
    { who: "tool", text: "$ echo hi-from-tool" },
    { who: "agent", text: "ALPHA, and the build is green." },
    { who: "person", text: "Now the pricing page." },
    { who: "agent", text: "Pricing is in: three tiers." },
  ],
};

let restoreLayout: () => void = () => {};
beforeAll(() => {
  restoreLayout = installFakeLayout();
});
afterAll(() => restoreLayout());
beforeEach(() => {
  useConversationsStore.setState({ answers: {}, held: {}, drafts: {}, previews: new Map(), picker: null, confirm: null });
  useComposerDraftStore.setState({ drafts: {}, queues: {}, held: {} });
  useComposerOptionsStore.setState({ byWorkspaceId: {}, pickedOn: {} });
  useMultiPickStore.setState({ byKey: {} });
});
afterEach(() => {
  cleanup();
  useStore.setState({ api: null, places: [], projects: [], projectHome: null, selectedId: null } as never);
});

/** An api that answers each project's list off `answer`, every read of one conversation with READ, and the start of a
 * thread on a conversation with `start`. */
function fakeApi(answer: ConversationsAnswer | Record<string, ConversationsAnswer>, start: (o: Record<string, unknown>) => Promise<unknown> = async o => ({ id: "s1", workspaceId: "ws_9", threadId: "th_9", harness: String(o["harness"]), status: "running" })) {
  const asked: Record<string, unknown>[] = [];
  const listed: string[] = [];
  const reads: { project: string; agent: string; id: string; preview?: boolean }[] = [];
  const claude: HarnessCatalog = { harness: "claude", label: "Claude Code", source: "table", version: null, models: [{ value: "claude-opus-5", label: "Opus 5", isDefault: true }], efforts: [], contextWindows: [], permissionModes: [], steers: true, renames: true, images: true };
  const api = {
    preferences: async () => DEFAULT_PREFERENCES,
    listHarnesses: async () => [claude],
    daemon: noDaemonApi,
    sessionHistory: async () => [],
    listWorkspaces: async () => [],
    listSessions: async () => [],
    watchStatuses: async () => [],
    subscribe: () => () => {},
    capabilities: async () => caps(),
    portReach: async () => ({ url: "https://x/", expiresAt: Date.now() + 1_000 }),
    getGolden: async () => undefined,
    listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
    snapshotStorage: async () => null,
    getWorkspace: async (id: string) => ({ id }),
    workspacesLanding: async () => ({ name: "studio", capabilities: caps() }),
    placesList: async () => ({ places: [HERE], adds: [] }),
    startSession: async (o: StartSessionOptions) => ({ id: "s", workspaceId: o.workspaceId, harness: "claude", status: "running" }),
    conversationsList: async (project: string) => {
      listed.push(project);
      return "rows" in answer ? answer : (answer[project] ?? { rows: [], held: [] });
    },
    conversationRead: async (o: { project: string; agent: string; id: string; preview?: boolean }) => {
      reads.push(o);
      return READ;
    },
    resumeConversation: async (o: Record<string, unknown>) => {
      asked.push(o);
      return start(o);
    },
  } as unknown as Api;
  return { api, asked, listed, reads };
}

function bind(api: Api) {
  useStore.setState({ conn: "connecting", workspaces: [], creations: [], statuses: {}, sessions: {}, harnesses: [], harnessesByWorkspace: {}, preferences: DEFAULT_PREFERENCES, projects: [LAB, SPOO], places: [HERE] } as never);
  useStore.getState().bind(api);
  useStore.getState().setConn("live");
}

describe("the palette's Resume a conversation", () => {
  it("opens the picker on the project on screen, and is held there with no project on screen", () => {
    const resumeConversation = vi.fn();
    const handlers = { selectThread: vi.fn(), resumeConversation } as never;
    const base = { projects: [], selectedId: null, query: "", messageHits: [], canCreate: true, recorded: [LAB], picks: [LAB], asks: false, handlers, verbs: {} as never, places: [HERE] };
    const held = buildPaletteItems({ ...base, resumeProject: null }).actionItems.find(i => i.title === CONVERSATION_WORDS.row)!;
    expect([held.kind, held.disabled, held.description]).toEqual(["action", true, CONVERSATION_WORDS.openProjectFirst]);
    const open = buildPaletteItems({ ...base, resumeProject: LAB }).actionItems.find(i => i.title === CONVERSATION_WORDS.row)!;
    expect([open.kind, open.disabled, open.description]).toEqual(["action", undefined, "lab"]);
    void (open as { run: () => Promise<void> }).run();
    expect(resumeConversation).toHaveBeenCalledWith(LAB.id);
  });

  it("is found by typing resume, and Enter opens the picker on that project", async () => {
    const { api, listed } = fakeApi(ANSWER);
    bind(api);
    useStore.setState({ projectHome: LAB.id } as never);
    render(
      <SidebarProvider>
        <CommandPalette />
        <ResumePicker />
      </SidebarProvider>,
    );
    act(() => openCommandPalette({ query: "resume" }));
    const input = await waitFor(() => document.querySelector<HTMLInputElement>("[data-command-palette] input")!);
    await waitFor(() => expect(screen.getByText(CONVERSATION_WORDS.row)).toBeDefined());
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(document.querySelector("[data-resume-picker]")).not.toBeNull());
    expect(useConversationsStore.getState().picker).toBe(LAB.id);
    expect(new Set(listed)).toEqual(new Set([LAB.id]));
  });
});

describe("the palette at launch, with no address", () => {
  it("offers Resume a conversation on the project the New thread page names, and Enter opens the picker on it", async () => {
    const { api, listed } = fakeApi(ANSWER);
    bind(api);
    useStore.setState({ projectHome: null, selectedId: null } as never);
    render(
      <SidebarProvider>
        <CommandPalette />
        <ResumePicker />
      </SidebarProvider>,
    );
    act(() => openCommandPalette({ query: "resume" }));
    const row = await screen.findByText(CONVERSATION_WORDS.row);
    const item = row.closest<HTMLElement>("[data-slot=command-item]");
    expect(item).not.toBeNull();
    expect(item!.textContent).toContain(LAB.name);
    expect(item!.textContent).not.toContain(CONVERSATION_WORDS.openProjectFirst);
    fireEvent.keyDown(document.querySelector<HTMLInputElement>("[data-command-palette] input[role=combobox]")!, { key: "Enter" });
    await waitFor(() => expect(useConversationsStore.getState().picker).toBe(LAB.id));
    expect(listed).toContain(LAB.id);
  });
});

describe("the project's menu", () => {
  it("offers Resume a conversation, which opens the picker on that project and reads its list", () => {
    const resumeConversation = vi.fn();
    const actions = resolveActions(projectActions, { id: LAB.id, name: LAB.name, workspaces: [] }, { newThread: vi.fn(), openSettings: vi.fn(), resumeConversation });
    const entry = actions.find(a => a.id === "resume-conversation")!;
    expect(entry.title).toBe(CONVERSATION_WORDS.menu);
    entry.run();
    expect(resumeConversation).toHaveBeenCalledWith(LAB.id);
    const { api, listed } = fakeApi(ANSWER);
    bind(api);
    openResumePicker(LAB.id);
    expect(useConversationsStore.getState().picker).toBe(LAB.id);
    expect(listed).toEqual([LAB.id]);
  });
});

describe("the picker's rows", () => {
  const all = ANSWER.rows.map(r => ({ project: LAB.id, row: r }));
  const titlesOf = (rows: ReturnType<typeof pickerRows>): string[] => rows.map(r => r.row.title);

  it("are newest first, found by title, first prompt, branch and agent's name", () => {
    const q = (query: string) => titlesOf(pickerRows(all, { agent: "all", branch: null, query }));
    expect(q("")).toEqual(["pricing page", "lab terminal thread", "lab codewords", "the old thread"]);
    expect(q("pricing")).toEqual(["pricing page"]);
    expect(q("ALPHA")).toEqual(["lab codewords"]);
    expect(q("feat/x")).toEqual(["lab codewords"]);
    expect(q("codex")).toEqual(["lab terminal thread"]);
  });

  it("keep to one agent, one branch, and over every project find a project by its name", () => {
    expect(titlesOf(pickerRows(all, { agent: "codex", branch: null, query: "" }))).toEqual(["lab terminal thread"]);
    expect(titlesOf(pickerRows(all, { agent: "all", branch: "feat/x", query: "" }))).toEqual(["lab codewords"]);
    const both = [...all, { project: SPOO.id, row: SPOO_ROW }];
    const names = new Map([
      [SPOO.id, "spoo"],
      [LAB.id, "lab"],
    ]);
    expect(titlesOf(pickerRows(both, { agent: "all", branch: null, query: "spoo", names }))).toEqual(["short links redirect twice"]);
  });

});

describe("the picker", () => {
  async function picker(answer: ConversationsAnswer | Record<string, ConversationsAnswer> = ANSWER) {
    const made = fakeApi(answer);
    bind(made.api);
    render(
      <SidebarProvider>
        <ResumePicker />
        <ConversationConfirm />
      </SidebarProvider>,
    );
    act(() => openResumePicker(LAB.id));
    return made;
  }
  const rowsShown = (): string[] => [...document.querySelectorAll<HTMLElement>("[data-resume-row]")].map(r => r.querySelector(".text-sm")!.textContent!);
  const rowText = (id: string): string => document.querySelector<HTMLElement>(`[data-resume-row="${id}"]`)!.textContent!.replace(/\s+/g, " ").trim();
  const input = (): HTMLInputElement => document.querySelector<HTMLInputElement>("[data-resume-picker] input")!;

  it("lists the project's conversations, each row its mark, title, what is true of it, branch, size and age, and the keys it takes", async () => {
    await picker();
    await waitFor(() => expect(rowsShown()).toEqual(["pricing page", "lab terminal thread", "lab codewords", "the old thread"]));
    expect(screen.getByText(CONVERSATION_WORDS.group("lab"))).toBeDefined();
    expect(rowText(TERMINAL.id)).toBe("lab codewordsfeat/x 1 MB2h");
    expect(rowText(OPEN.id)).toBe(`pricing page${CONVERSATION_WORDS.live}1m`);
    expect(rowText(THREADED.id)).toBe(`the old thread${CONVERSATION_WORDS.thread}1d`);
    expect(document.querySelector(`[data-resume-row="${CODEX.id}"] [data-harness-mark="codex"]`)).not.toBeNull();
    expect(document.querySelector("[data-resume-picker]")!.textContent).not.toMatch(/\d+ messages|in this folder/);
    const footer = document.querySelector<HTMLElement>("[data-slot=command-footer]")!.textContent!;
    for (const word of [CONVERSATION_WORDS.navigate, CONVERSATION_WORDS.send, CONVERSATION_WORDS.copy, CONVERSATION_WORDS.close]) expect(footer).toContain(word);
  });

  const drawn = (): (string | null)[] => [...document.querySelectorAll<HTMLElement>("[data-resume-row][data-highlighted]")].map(r => r.getAttribute("data-resume-row"));

  it("holds the highlight on the first row at rest, the first down press moves it to the second, and Enter takes the row drawn", async () => {
    await picker({ rows: [TERMINAL, OPEN, CODEX], held: [] });
    await waitFor(() => expect(drawn()).toEqual([OPEN.id]));
    fireEvent.keyDown(input(), { key: "ArrowDown" });
    await waitFor(() => expect(drawn()).toEqual([CODEX.id]));
    fireEvent.keyDown(input(), { key: "Enter" });
    await waitFor(() => expect(useConversationsStore.getState().held[LAB.id]).toEqual({ row: CODEX, copy: false }));
  });

  it("takes the first match on Enter after a search", async () => {
    await picker({ rows: [TERMINAL, OPEN, CODEX], held: [] });
    await waitFor(() => expect(drawn()).toEqual([OPEN.id]));
    fireEvent.keyDown(input(), { key: "ArrowDown" });
    fireEvent.change(input(), { target: { value: "codewords" } });
    await waitFor(() => expect(drawn()).toEqual([TERMINAL.id]));
    fireEvent.keyDown(input(), { key: "Enter" });
    await waitFor(() => expect(useConversationsStore.getState().held[LAB.id]).toEqual({ row: TERMINAL, copy: false }));
  });

  it("keeps the drawn row with the pointer off the rows, and Enter takes that row", async () => {
    await picker({ rows: [TERMINAL, CODEX], held: [] });
    await waitFor(() => expect(drawn()).toEqual([CODEX.id]));
    fireEvent.mouseMove(document.querySelector<HTMLElement>("[data-slot=command-group-label]")!);
    fireEvent.mouseMove(document.querySelector<HTMLElement>("[data-slot=command-list]")!);
    await waitFor(() => expect(drawn()).toEqual([CODEX.id]));
    fireEvent.keyDown(input(), { key: "Enter" });
    await waitFor(() => expect(useConversationsStore.getState().held[LAB.id]).toEqual({ row: CODEX, copy: false }));
  });

  it("narrows to what is typed", async () => {
    await picker();
    await waitFor(() => expect(rowsShown()).toHaveLength(4));
    fireEvent.change(input(), { target: { value: "ALPHA" } });
    await waitFor(() => expect(rowsShown()).toEqual(["lab codewords"]));
    fireEvent.change(input(), { target: { value: "nothing like it" } });
    await waitFor(() => expect(screen.getByText(CONVERSATION_WORDS.noMatch)).toBeDefined());
  });

  it("offers the agent menu only where two agents have rows, and over every project lists every project's rows in one list newest first", async () => {
    await picker({ [LAB.id]: ANSWER, [SPOO.id]: { rows: [SPOO_ROW], held: [] } });
    await waitFor(() => expect(rowsShown()).toHaveLength(4));
    expect(document.querySelector("[data-resume-scope=agent]")!.textContent).toBe(CONVERSATION_WORDS.allAgents);
    expect(document.querySelector("[data-resume-scope=project]")!.textContent).toBe("lab");
    fireEvent.click(document.querySelector<HTMLElement>("[data-resume-scope=project]")!);
    fireEvent.click(await screen.findByRole("menuitemradio", { name: CONVERSATION_WORDS.allProjects }));
    await waitFor(() => expect(rowsShown()).toEqual(["pricing page", "short links redirect twice", "lab terminal thread", "lab codewords", "the old thread"]));
    expect(screen.getByText(CONVERSATION_WORDS.everyProject)).toBeDefined();
    expect(rowText(SPOO_ROW.id)).toBe("short links redirect twicespoo3m");
    cleanup();
    await picker({ rows: [TERMINAL], held: [] });
    await waitFor(() => expect(rowsShown()).toHaveLength(1));
    expect(document.querySelector("[data-resume-scope=agent]")).toBeNull();
  });

  it("previews the highlighted row once the highlight rests, its folder from the home and its newest four messages", async () => {
    const { reads } = await picker();
    await waitFor(() => expect(rowsShown()).toHaveLength(4));
    await waitFor(() => expect(document.querySelector("[data-resume-preview]")!.textContent).toContain("Pricing is in: three tiers."));
    expect(reads).toEqual([{ project: LAB.id, agent: "claude", id: OPEN.id, preview: true }]);
    const pane = document.querySelector<HTMLElement>("[data-resume-preview]")!;
    expect(pane.textContent).toContain(`${CONVERSATION_WORDS.runsIn} /home/dev/work/acme/lab-wt`);
    expect(pane.textContent).not.toContain("Remember the codeword ALPHA.");
    expect(pane.textContent).toContain("$ echo hi-from-tool");
    expect(pane.textContent).not.toContain(earlierLine(41, "Claude Code"));
    expect(pane.className).toContain("hidden");
    expect(pane.className).toContain("md:flex");
  });

  it("says where a conversation runs from the home on its computer's row, this computer's as the host now sends it", async () => {
    for (const [cwd, said] of [
      ["/srv/dev/work/acme/lab-wt", "~/work/acme/lab-wt"],
      ["/home/other/lab", "/home/other/lab"],
    ]) {
      const { api } = fakeApi(ANSWER);
      bind({ ...api, placesList: async () => ({ places: [{ ...HERE, home: "/srv/dev" }], adds: [] }), conversationRead: async () => ({ ...READ, cwd }) } as Api);
      await waitFor(() => expect(useStore.getState().places[0]?.home).toBe("/srv/dev"));
      render(
        <SidebarProvider>
          <ResumePicker />
        </SidebarProvider>,
      );
      act(() => openResumePicker(LAB.id));
      await waitFor(() => expect(document.querySelector("[data-resume-preview]")!.textContent).toContain(`${CONVERSATION_WORDS.runsIn} ${said}`));
      cleanup();
      useConversationsStore.setState({ previews: new Map(), picker: null });
    }
  });

  it("asks for no preview of a row the highlight only passes over", async () => {
    const { reads } = await picker();
    await waitFor(() => expect(rowsShown()).toHaveLength(4));
    for (let i = 0; i < 3; i++) {
      fireEvent.keyDown(input(), { key: "ArrowDown" });
      await new Promise(r => setTimeout(r, PREVIEW_REST_MS / 3));
    }
    await new Promise(r => setTimeout(r, PREVIEW_REST_MS + 50));
    expect(reads).toHaveLength(1);
    expect(reads[0]!.id).not.toBe(OPEN.id);
  });

  it("draws the newest rows of a long list first and more as the highlight nears the end, searching every row", async () => {
    const many = Array.from({ length: 150 }, (_, i) => row({ id: `00000000-0000-4000-8000-${String(i).padStart(12, "0")}`, title: `conversation ${i}`, lastAt: Date.now() - i * 60_000 }));
    await picker({ rows: many, held: [] });
    await waitFor(() => expect(rowsShown()).toHaveLength(ROWS_DRAWN));
    for (let i = 0; i < ROWS_DRAWN - 4; i++) fireEvent.keyDown(input(), { key: "ArrowDown" });
    await waitFor(() => expect(rowsShown().length).toBeGreaterThan(ROWS_DRAWN));
    fireEvent.change(input(), { target: { value: "conversation 149" } });
    await waitFor(() => expect(rowsShown()).toEqual(["conversation 149"]));
  });

  it("asks again for a preview the host refused, keeping only what it read", async () => {
    const { api } = fakeApi(ANSWER);
    let refuse = true;
    const asked: string[] = [];
    bind({ ...api, conversationRead: async (o: { id: string }) => {
      asked.push(o.id);
      if (refuse) throw new Error("studio is not answering");
      return READ;
    } } as Api);
    await useConversationsStore.getState().preview(LAB.id, TERMINAL);
    expect(useConversationsStore.getState().previews.get(previewKey(LAB.id, TERMINAL))).toMatchObject({ said: "studio is not answering" });
    refuse = false;
    await useConversationsStore.getState().preview(LAB.id, TERMINAL);
    await useConversationsStore.getState().preview(LAB.id, TERMINAL);
    expect(asked).toEqual([TERMINAL.id, TERMINAL.id]);
    expect(useConversationsStore.getState().previews.get(previewKey(LAB.id, TERMINAL))).toEqual(READ);
  });

  it("keeps the newest twenty previews and lets the oldest go", async () => {
    const { api } = fakeApi(ANSWER);
    bind(api);
    for (let i = 0; i < PREVIEWS_KEPT + 5; i++) await useConversationsStore.getState().preview(LAB.id, { ...TERMINAL, id: `id-${i}` });
    const kept = [...useConversationsStore.getState().previews.keys()];
    expect(kept).toHaveLength(PREVIEWS_KEPT);
    expect(kept[0]).toContain("id-5");
  });

  it("says so where the project has none, with only the project menu and no Enter", async () => {
    await picker({ rows: [], held: [] });
    await waitFor(() => expect(screen.getByText(CONVERSATION_WORDS.emptyIn("lab"))).toBeDefined());
    expect([...document.querySelectorAll("[data-resume-scope]")].map(e => e.getAttribute("data-resume-scope"))).toEqual(["project"]);
    const footer = document.querySelector<HTMLElement>("[data-slot=command-footer]")!.textContent!;
    expect(footer).not.toContain(CONVERSATION_WORDS.send);
    expect(footer).not.toContain(CONVERSATION_WORDS.copy);
  });

  it("says in one line with its mark which agent was not read and why, the others' rows listed and no count", async () => {
    const said = "Claude Code's conversations could not be read: at most 64 entries, got 74";
    await picker({ rows: [CODEX], held: [{ agent: "claude", said, fix: "Ask again in a moment.", why: "at most 64 entries, got 74" }] });
    const line = await waitFor(() => document.querySelector<HTMLElement>("[data-resume-held]")!);
    expect(line.textContent).toBe(said);
    expect(line.querySelector('[data-harness-mark="claude"]')).not.toBeNull();
    await waitFor(() => expect(rowsShown()).toEqual(["lab terminal thread"]));
  });

  it("opens the highlighted row as a draft on Enter and a copy on ⇧Enter, and asks first about one open elsewhere", async () => {
    await picker({ rows: [TERMINAL, OPEN], held: [] });
    await waitFor(() => expect(rowsShown()).toEqual(["pricing page", "lab codewords"]));
    fireEvent.keyDown(input(), { key: "Enter" });
    const dialog = await waitFor(() => document.querySelector<HTMLElement>("[data-k=conversation-open]")!);
    expect(within(dialog).getByText(CONVERSATION_WORDS.openTitle("pricing page", "studio"))).toBeDefined();
    expect(useConversationsStore.getState().held[LAB.id]).toBeUndefined();
    await waitFor(() => expect(document.querySelector("[data-resume-picker]")).toBeNull());
    fireEvent.click(dialog.querySelector<HTMLElement>("[data-k=conversation-copy]")!);
    await waitFor(() => expect(useConversationsStore.getState().held[LAB.id]).toEqual({ row: OPEN, copy: true }));
    expect(useStore.getState().projectHome).toBe(LAB.id);

    act(() => openResumePicker(LAB.id));
    await waitFor(() => expect(rowsShown()).toHaveLength(2));
    fireEvent.keyDown(input(), { key: "Enter", shiftKey: true });
    await waitFor(() => expect(useConversationsStore.getState().picker).toBeNull());
    expect(useConversationsStore.getState().held[LAB.id]).toEqual({ row: OPEN, copy: true });
    expect(useConversationsStore.getState().confirm).toBeNull();

    act(() => openResumePicker(LAB.id));
    await waitFor(() => expect(rowsShown()).toHaveLength(2));
    fireEvent.keyDown(input(), { key: "ArrowDown" });
    fireEvent.keyDown(input(), { key: "Enter" });
    await waitFor(() => expect(useConversationsStore.getState().held[LAB.id]).toEqual({ row: TERMINAL, copy: false }));
    expect(useConversationsStore.getState().picker).toBeNull();
  });
});

describe("a draft opened off the picker", () => {
  it("takes the focus into its composer, on Enter and on Continue in a copy", async () => {
    for (const row of [TERMINAL, OPEN]) {
      const { api } = fakeApi({ rows: [row], held: [] });
      bind(api);
      render(
        <SidebarProvider>
          <button type="button" data-k="project-switcher">
            lab
          </button>
          <ProjectHome projectId={LAB.id} />
          <ResumePicker />
          <ConversationConfirm />
        </SidebarProvider>,
      );
      // Opened off the project's menu, whose trigger holds the focus as the picker opens.
      document.querySelector<HTMLElement>("[data-k=project-switcher]")!.focus();
      act(() => openResumePicker(LAB.id));
      await waitFor(() => expect(document.querySelector(`[data-resume-row="${row.id}"][data-highlighted]`)).not.toBeNull());
      fireEvent.keyDown(document.querySelector<HTMLInputElement>("[data-resume-picker] input")!, { key: "Enter" });
      if (row.live) fireEvent.click(await waitFor(() => document.querySelector<HTMLElement>("[data-k=conversation-copy]")!));
      await waitFor(() => expect(document.querySelector("[data-k=resume-draft]")).not.toBeNull());
      await waitFor(() => expect(document.querySelector("[data-resume-picker]")).toBeNull());
      await new Promise(r => setTimeout(r, 200));
      expect(document.querySelector("[data-k=resume-draft] [data-chat-composer]")!.contains(document.activeElement)).toBe(true);
      cleanup();
      useConversationsStore.setState({ held: {}, drafts: {}, confirm: null, picker: null });
    }
  });
});

describe("a row a wsp thread already runs on", () => {
  it("opens that thread on Enter and on ⇧Enter, holding no draft", async () => {
    const ws: WorkspaceView = { id: "ws_1", name: "lab", machineId: "m1", project: { id: LAB.id, name: LAB.name, path: LAB.path, computer: "here" }, phase: "running", createdAt: "2026-09-01T00:00:00Z", kind: "local", folder: LAB.path } as WorkspaceView;
    const session: SessionView = { id: "sess_7f", workspaceId: "ws_1", harness: "claude", status: "completed", threadId: "thread-7f", claudeSessionId: THREADED.id, prompt: "the old thread" };
    for (const shiftKey of [false, true]) {
      const { api } = fakeApi({ rows: [THREADED], held: [] });
      bind({ ...api, listWorkspaces: async () => [ws], listSessions: async () => [session] } as Api);
      await waitFor(() => expect(useStore.getState().sessions["ws_1"]).toHaveLength(1));
      render(
        <SidebarProvider>
          <ResumePicker />
        </SidebarProvider>,
      );
      act(() => openResumePicker(LAB.id));
      await waitFor(() => expect(document.querySelector(`[data-resume-row="${THREADED.id}"]`)).not.toBeNull());
      fireEvent.keyDown(document.querySelector<HTMLInputElement>("[data-resume-picker] input")!, { key: "Enter", shiftKey });
      await waitFor(() => expect(useStore.getState().selectedId).toBe("ws_1"));
      expect(useConversationsStore.getState().held[LAB.id]).toBeUndefined();
      cleanup();
      useStore.setState({ selectedId: null } as never);
    }
  });
});

describe("a project's New thread page", () => {
  async function home(api: Api) {
    render(
      <>
        <ProjectHome projectId={LAB.id} />
        <ConversationConfirm />
      </>,
    );
    await waitFor(() => expect(composerEditor()).toBeDefined());
    return api;
  }

  it("carries no line about conversations under its composer and reads none", async () => {
    const { api, listed, reads } = fakeApi(ANSWER);
    bind(api);
    await home(api);
    await new Promise(r => setTimeout(r, 50));
    expect(listed).toEqual([]);
    expect(reads).toEqual([]);
    expect(document.querySelector("[data-k=conversations-offer]")).toBeNull();
    expect(document.querySelector("[data-k=project-home]")!.textContent).not.toContain(CONVERSATION_WORDS.row);
  });

  it("holding a draft, shows the conversation shut in its Earlier fold, opens it, and sends on it under its own agent", async () => {
    const { api, asked, reads } = fakeApi(ANSWER);
    bind(api);
    act(() => useConversationsStore.getState().hold(LAB.id, { row: TERMINAL, copy: false }));
    await home(api);
    expect(reads).toEqual([{ project: LAB.id, agent: "claude", id: TERMINAL.id }]);
    const fold = await screen.findByRole("button", { name: CONVERSATION_WORDS.earlierFold });
    expect(fold.getAttribute("aria-expanded")).toBe("false");
    expect(document.body.textContent).not.toContain("Pricing is in: three tiers.");
    fireEvent.click(fold);
    await waitFor(() => expect(document.body.textContent).toContain("Pricing is in: three tiers."));
    expect(document.body.textContent).toContain(earlierLine(41, "Claude Code"));
    expect(document.querySelector("[data-resume-banner]")!.textContent).toContain(CONVERSATION_WORDS.continues("lab codewords", false));
    expect(screen.getByRole("button", { name: CONVERSATION_WORDS.send })).toBeDefined();
    const editor = composerEditor();
    await typeInto(editor, "and now the tests");
    await press(editor, "Enter");
    await waitFor(() => expect(asked).toHaveLength(1));
    expect(asked[0]).toMatchObject({ project: LAB.id, prompt: "and now the tests", harness: "claude", resume: { id: TERMINAL.id } });
    expect(asked[0]!["resume"]).not.toHaveProperty("copy");
    await waitFor(() => expect(useStore.getState().selectedId).toBe("ws_9"));
    expect(useConversationsStore.getState().held[LAB.id]).toBeUndefined();
    expect(useConversationsStore.getState().drafts[LAB.id]).toBeUndefined();
  });

  it("lets a draft go with Start fresh, back to the page as it was", async () => {
    const { api } = fakeApi(ANSWER);
    bind(api);
    act(() => useConversationsStore.getState().hold(LAB.id, { row: TERMINAL, copy: true }));
    await home(api);
    expect(document.querySelector("[data-resume-banner]")!.textContent).toContain(CONVERSATION_WORDS.continues("lab codewords", true));
    fireEvent.click(document.querySelector<HTMLElement>("[data-k=resume-dismiss]")!);
    await waitFor(() => expect(document.querySelector("[data-k=resume-draft]")).toBeNull());
    expect(document.querySelector("[data-k=project-home]")).not.toBeNull();
  });

  it("opens the confirm on a send the agent refused as open elsewhere, keeps the message, and tries again or copies", async () => {
    let refuse = true;
    const { api, asked } = fakeApi(ANSWER, async o => {
      if (refuse && (o["resume"] as { copy?: boolean }).copy !== true) throw Object.assign(refusal(conversationOpenLine("lab terminal thread", "studio"), conversationOpenFix(CODEX.letsGo), CONVERSATION_OPEN_KIND), {});
      return { id: "s2", workspaceId: "ws_9", threadId: "th_9", harness: "codex", status: "running" };
    });
    bind(api);
    act(() => useConversationsStore.getState().hold(LAB.id, { row: CODEX, copy: false }));
    await home(api);
    const editor = composerEditor();
    await typeInto(editor, "keep going");
    await press(editor, "Enter");
    const dialog = await waitFor(() => document.querySelector<HTMLElement>("[data-k=conversation-open]")!);
    expect(useComposerDraftStore.getState().drafts[HOME]?.prompt).toBe("keep going");
    expect(within(dialog).getByText(CONVERSATION_WORDS.openNote(CODEX.letsGo))).toBeDefined();
    fireEvent.click(dialog.querySelector<HTMLElement>("[data-k=conversation-try-again]")!);
    await waitFor(() => expect(asked).toHaveLength(2));
    expect(document.querySelector("[data-k=conversation-open]")).not.toBeNull();
    refuse = false;
    fireEvent.click(document.querySelector<HTMLElement>("[data-k=conversation-copy]")!);
    await waitFor(() => expect(asked).toHaveLength(3));
    expect(asked[2]).toMatchObject({ prompt: "keep going", harness: "codex", resume: { id: CODEX.id, copy: true } });
    await waitFor(() => expect(document.querySelector("[data-k=conversation-open]")).toBeNull());
    expect(useComposerDraftStore.getState().drafts[HOME]?.prompt).toBe("");
  });
});

describe("a thread opened on a conversation", () => {
  const scope = { workspaceId: "ws_9", sessionId: TERMINAL.id, threadId: "th_9" };
  const events: SessionEvent[] = [
    { type: "session.earlier", ...scope, who: "note", text: "3 earlier messages stay in Claude Code's own history", at: 1, pos: 1 },
    { type: "session.earlier", ...scope, who: "person", text: "Remember ALPHA.", at: 2, pos: 2 },
    { type: "session.earlier", ...scope, who: "tool", text: "$ echo hi", at: 3, pos: 3 },
    { type: "session.earlier", ...scope, who: "agent", text: "ALPHA", at: 4, pos: 4 },
    { type: "session.start", ...scope, turnId: "t1", prompt: "go on", at: 5, pos: 5 },
  ];

  it("draws the earlier messages as messages and lines of no turn, ahead of its first turn", () => {
    const model = deriveSession(events);
    expect(model.timeline.map(e => (e.kind === "message" ? `${e.message.role}: ${e.message.text}` : e.kind === "work" ? `work: ${e.entry.label}` : e.kind))).toEqual([
      "work: 3 earlier messages stay in Claude Code's own history",
      "user: Remember ALPHA.",
      "work: $ echo hi",
      "assistant: ALPHA",
      "user: go on",
    ]);
    expect(model.turns.map(t => t.turnId)).toEqual(["t1"]);
  });

  it("keeps them shut in the Earlier fold each time its transcript is read, open once the fold is, and find opens it to a match", () => {
    const model = deriveSession(events);
    const input = { timelineEntries: model.timeline, turns: model.turns, isWorking: false, activeTurnStartedAt: null };
    const shut = deriveMessagesTimelineRows(input);
    expect(shut.map(r => (r.kind === "turn-fold" ? `fold: ${r.label} ${String(r.expanded)}` : r.kind === "message" ? r.message.text : r.kind))).toEqual([`fold: ${CONVERSATION_WORDS.earlierFold} false`, "go on"]);
    const open = deriveMessagesTimelineRows({ ...input, expandedTurnIds: new Set([EARLIER_FOLD]) });
    expect(open.filter(r => r.kind === "message").map(r => (r.kind === "message" ? r.message.text : ""))).toEqual(["Remember ALPHA.", "ALPHA", "go on"]);
    const person = model.timeline.find(e => e.kind === "message" && e.message.text === "Remember ALPHA.")!;
    expect(entryHiders(input, person.id).turnId).toBe(EARLIER_FOLD);
  });
});

describe("the words about conversations", () => {
  it("say conversation, never session", () => {
    const said = Object.values(CONVERSATION_WORDS).map(w => (typeof w === "function" ? (w as (...a: unknown[]) => string)("x", "y") : w));
    for (const words of said) expect(words).not.toMatch(/session/i);
  });
});
