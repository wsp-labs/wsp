// SPDX-License-Identifier: AGPL-3.0-only
// Design round for #2054, a prototype to click, never shipped. Served by Vite to a real browser: the app shell over a
// fake host on zingzy's MacBook Pro with wsp, spoo and spoo-landing (on hetzner), and the Resume picker the research
// recommends in place of the palette's conversations page. ?screen=live (the default) opens nothing: Ctrl or Cmd+K,
// type resume, Enter; or right-click wsp in the sidebar's project picker, then Resume a conversation... Every other
// screen opens on one surface: today (the line under the composer that goes), palette, menu, plan (the picker as the
// ticket wrote it), rec, all, empty, unread, confirm. ?theme=light for Paper; ?bar=0 hides the switcher for a shot.
// Enter holds the pick on the New thread page as main does today; the draft thread with its Earlier fold is not drawn.
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { create } from "zustand";
import { DEFAULT_PREFERENCES, type ConversationsAnswer, type ConversationsHeld, type HarnessCatalog, type ProjectView, type SessionView, type WorkspaceView } from "@wsp/protocol";
import { isCommandPaletteOpen, openCommandPalette } from "../../src/commandPaletteBus";
import { Button } from "../../src/components/ui/button";
import { TooltipProvider } from "../../src/components/ui/tooltip";
import type { Api } from "../../src/protocol/client";
import { useHomeProject, useSelectedId, useSelectedThreadId, useStore } from "../../src/protocol/store";
import { applyTheme } from "../../src/settings/theme";
import { AppShell } from "../../src/shell/AppShell";
import { useConversationsStore } from "../../src/shell/conversations";
import { ProjectHome } from "../../src/shell/ProjectHome";
import { WorkspaceThread } from "../../src/shell/WorkspaceThread";
import { PROJECT_PICK_KEY } from "../../src/sidebar/picks";
import { statusOf } from "../workspace-status";
import { caps } from "../caps.js";
import { noDaemonApi } from "../fake-daemon-api.js";
import { ACCESS_MODES } from "../fixtures/access-modes";
import { ResumePicker, type PickerProject, type Variant } from "./ResumePicker";
import { OTHER_ROWS, WSP_ROWS, type PickRow } from "./rows";
import "../../src/index.css";
import "../../src/themes/index";

const params = new URLSearchParams(window.location.search);
const theme: "light" | "dark" = params.get("theme") === "light" ? "light" : "dark";
const picks = { lightTheme: DEFAULT_PREFERENCES.lightTheme, darkTheme: DEFAULT_PREFERENCES.darkTheme };
applyTheme({ theme, ...picks }, theme === "dark");
const screen = params.get("screen") ?? "live";
// The project's right-click menu hangs on the picker's own row only while one project is picked (ticket 4).
if (screen === "menu" || screen === "live") window.localStorage.setItem(PROJECT_PICK_KEY, JSON.stringify("pr_wsp"));
else window.localStorage.removeItem(PROJECT_PICK_KEY);

const recorded = (id: string, name: string, computer: string, path: string): ProjectView => ({ id, name, computer, source: { kind: "folder" as const, path }, path, remote: "", defaultBranch: "main", memoryKey: `-${name}`, memoryDir: "/m", createdAt: "t" }) as ProjectView;
const PROJECTS = [recorded("pr_wsp", "wsp", "here", "/Users/zingzy/wsp"), recorded("pr_spoo", "spoo", "here", "/Users/zingzy/spoo"), recorded("pr_landing", "spoo-landing", "p_hetzner", "/root/spoo-landing")];
const PICKER_PROJECTS: PickerProject[] = [
  { id: "pr_wsp", name: "wsp", root: "/Users/zingzy/wsp" },
  { id: "pr_spoo", name: "spoo", root: "/Users/zingzy/spoo" },
  { id: "pr_landing", name: "spoo-landing", root: "/root/spoo-landing", on: "hetzner" },
];
const PLACES = [
  { id: "here", kind: "computer" as const, name: "zingzy-mbp", label: "zingzy's MacBook Pro", default: true, present: true, mac: "macbook" as const, takesForks: false },
  { id: "p_hetzner", kind: "computer" as const, name: "hetzner", default: false, present: true, takesForks: true },
];
const MAC: WorkspaceView = { id: "ws_m", name: "wsp", kind: "local", machineId: "local", project: { id: "pr_wsp", name: "wsp", path: "/Users/zingzy/wsp", computer: "here" }, phase: "running", golden: "", createdAt: "2026-10-09T09:00:00Z", place: "here" } as WorkspaceView;
const SPOO: WorkspaceView = { id: "ws_s", name: "spoo", kind: "local", machineId: "local", project: { id: "pr_spoo", name: "spoo", path: "/Users/zingzy/spoo", computer: "here" }, phase: "running", golden: "", createdAt: "2026-10-09T09:00:00Z", place: "here" } as WorkspaceView;
const workspaces = [MAC, SPOO];
const MIN = 60_000;
const ago = (ms: number): number => Date.now() - ms;
const sessions: SessionView[] = [
  { id: "s1", threadId: "s1", workspaceId: "ws_m", harness: "claude", status: "running", prompt: "Design: Resume picker (#2054)", startedBy: "person", startedAt: ago(38 * MIN), cwd: "/Users/zingzy/wsp" },
  { id: "s2", threadId: "s2", workspaceId: "ws_m", harness: "claude", status: "completed", prompt: "Build: resume reads every Claude Code folder (#2049)", startedBy: "person", startedAt: ago(140 * MIN), endedAt: ago(95 * MIN), readAt: ago(90 * MIN), cwd: "/Users/zingzy/wsp" },
  { id: "s3", threadId: "s3", workspaceId: "ws_m", harness: "codex", status: "completed", prompt: "Review 2049 round 1", startedBy: "person", startedAt: ago(80 * MIN), endedAt: ago(70 * MIN), readAt: ago(60 * MIN), cwd: "/Users/zingzy/wsp" },
  { id: "s4", threadId: "s4", workspaceId: "ws_s", harness: "claude", status: "completed", prompt: "Short link analytics export", startedBy: "person", startedAt: ago(300 * MIN), endedAt: ago(280 * MIN), readAt: ago(270 * MIN), cwd: "/Users/zingzy/spoo" },
  // The conversation the picker's "Already a thread" row runs on.
  { id: "s5", threadId: "s5", workspaceId: "ws_m", harness: "claude", status: "completed", prompt: "Resume picker plan", startedBy: "person", startedAt: ago(320 * MIN), endedAt: ago(300 * MIN), readAt: ago(290 * MIN), cwd: "/Users/zingzy/wsp" },
] as SessionView[];
const catalogs: HarnessCatalog[] = [
  { harness: "claude", label: "Claude Code", source: "harness", version: "2.1.296", models: [{ value: "claude-opus-5-5", label: "Opus 5.5", isDefault: true, contextWindows: [] }], efforts: [], contextWindows: [], permissionModes: ACCESS_MODES.map(o => (o.value === "bypassPermissions" ? { ...o, isDefault: true } : o)), steers: true, renames: true, images: true, access: { ask: "default", "auto-edit": "acceptEdits", full: "bypassPermissions" }, bypassMode: "bypassPermissions" } as HarnessCatalog,
];
/** The owner's wsp folder held 48 rows on 2026-10-11; the screenful above plus older ones make the count his. */
const filler: PickRow[] = Array.from({ length: 48 - WSP_ROWS.length }, (_, i) => ({ ...WSP_ROWS[11]!, id: `old-${i}`, title: `Older conversation ${i + 1}`, lastAt: ago((12 + i) * 24 * 60 * MIN) }));
const answer: ConversationsAnswer = { rows: [...WSP_ROWS, ...filler], held: [] };

const api: Api = {
  listWorkspaces: async () => workspaces,
  getWorkspace: async id => workspaces.find(w => w.id === id)!,
  createWorkspace: async () => workspaces[0]!,
  watchStatuses: async () => workspaces.map(w => statusOf(w, { kind: "local", size: { cpu: 10, memMb: 16384 }, rateUsdPerHour: 0, facts: { os: "macOS 15.5", uptimeMs: 3 * 86_400_000, folder: w.project.path } })),
  forget: async () => {},
  nap: async id => workspaces.find(w => w.id === id)!,
  wake: async id => workspaces.find(w => w.id === id)!,
  capabilities: async () => caps(),
  startSession: async o => ({ id: "s9", workspaceId: o.workspaceId, harness: "claude", status: "running" }),
  portReach: async (_id, port) => ({ url: `https://m1-${port}.preview.example/?pt_token=e`, expiresAt: Date.now() + 3_600_000 }),
  daemon: noDaemonApi,
  sessionHistory: async () => [],
  listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
  snapshotStorage: async () => null,
  rollbackSnapshot: async () => ({ lineage: { name: "default", head: null, versions: [] }, existingWorkspaces: "untouched" }),
  listSessions: async () => sessions,
  listHarnesses: async () => catalogs,
  subscribe: () => () => {},
  getGolden: async () => undefined,
  listProjectGoldens: async () => [],
  projectsList: async () => PROJECTS,
  placesList: async () => ({ places: PLACES, adds: [] }),
  // Only today's page answers, so its line draws; everywhere else the line is gone, as the plan has it.
  conversationsList: async () => (screen === "today" ? answer : new Promise<ConversationsAnswer>(() => {})),
} as Api;

useStore.setState({ conn: "live" });
useStore.getState().bind(api);
useStore.setState(s => ({ places: PLACES, projects: PROJECTS, preferences: { ...DEFAULT_PREFERENCES, ...s.preferences, theme, ...picks, projectLook: { pr_wsp: { icon: "rocket", hue: "teal" }, pr_spoo: { icon: "globe", hue: "violet" } } } }));
useStore.getState().openProjectHome("pr_wsp");

const ALL_ROWS = [...WSP_ROWS, ...OTHER_ROWS];
const UNREAD: ConversationsHeld[] = [{ agent: "claude", said: "the daemon on zingzy's MacBook Pro is too old to list them", fix: "Update it from Computers." }];

/** The picker as the switcher and the entry points leave it; each open draws a fresh one, its search empty. */
interface PickerState {
  readonly open: boolean;
  readonly opens: number;
  readonly variant: Variant;
  readonly data: "full" | "empty" | "unread";
  readonly scope: "this" | "all";
  readonly project: string;
}
const usePicker = create<PickerState>(() => ({
  open: ["plan", "rec", "rec-pick", "all", "empty", "unread", "reading"].includes(screen),
  opens: 0,
  variant: screen === "plan" ? "plan" : "rec",
  data: screen === "empty" ? "empty" : screen === "unread" ? "unread" : "full",
  scope: screen === "all" ? "all" : "this",
  project: "pr_wsp",
}));
const openPicker = (over: Partial<PickerState> = {}): void => usePicker.setState(s => ({ open: true, opens: s.opens + 1, variant: "rec", data: "full", scope: "this", ...over }));

// The project menu's entry asks the palette for its conversations page; the picker takes the ask in its place.
window.addEventListener(
  "wsp:open-command-palette",
  event => {
    const detail = (event as CustomEvent<{ page?: string; project?: string }>).detail;
    if (detail?.page !== "conversations") return;
    event.stopImmediatePropagation();
    openPicker({ project: detail.project ?? useStore.getState().projectHome ?? "pr_wsp" });
  },
  { capture: true },
);
// The palette's own row opens its page inside the palette; the picker takes over the moment that page draws, once
// per page, since the page stays in the document while the palette animates shut.
let taken = false;
new MutationObserver(() => {
  if (document.querySelector('[data-command-palette] input[placeholder="Search conversations..."]') === null) {
    taken = false;
    return;
  }
  if (taken) return;
  taken = true;
  if (isCommandPaletteOpen()) openCommandPalette({ toggle: true });
  openPicker({ project: useStore.getState().projectHome ?? "pr_wsp" });
}).observe(document.body, { childList: true, subtree: true });

/** What a pick does here: a wsp thread opens; a row open in another app asks first with the real confirm; any other is
 * held on its project's New thread page, as main does today, on a copy for ⇧Enter. */
function pick(row: PickRow, copy: boolean): void {
  usePicker.setState({ open: false });
  const store = useStore.getState();
  if (row.thread !== undefined) {
    const held = sessions.find(s => s.threadId === row.thread);
    if (held !== undefined) store.select(held.workspaceId, held.threadId ?? null);
    return;
  }
  if (row.live && !copy) {
    useConversationsStore.getState().ask({ project: row.project, row });
    return;
  }
  useConversationsStore.getState().hold(row.project, { row, copy });
  store.openProjectHome(row.project);
}

function Picker() {
  const p = usePicker();
  const rows = p.data === "empty" ? [] : p.data === "unread" ? ALL_ROWS.filter(r => r.agent === "codex") : ALL_ROWS;
  return (
    <ResumePicker
      key={p.opens}
      open={p.open}
      onClose={() => usePicker.setState({ open: false })}
      onPick={pick}
      variant={p.variant}
      rows={rows}
      projects={PICKER_PROJECTS}
      project={p.project}
      thisBranch="experimental"
      projectScope={p.scope}
      {...(p.data === "unread" ? { held: UNREAD } : {})}
      {...(screen === "reading" ? { previewReading: "8f1c2a90-1" } : {})}
    />
  );
}

/** The centre as App draws it: the thread on screen, else the project's New thread page. */
function Centre() {
  const workspaceId = useSelectedId();
  const threadId = useSelectedThreadId();
  const home = useHomeProject()?.id ?? "pr_wsp";
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-terminal-beside>
      {workspaceId === null ? <ProjectHome key={home} projectId={home} /> : <WorkspaceThread workspaceId={workspaceId} threadId={threadId} />}
    </div>
  );
}

const SCREENS: { screen: string; label: string }[] = [
  { screen: "live", label: "Live" },
  { screen: "rec", label: "Recommended" },
  { screen: "plan", label: "Plan as written" },
  { screen: "all", label: "All projects" },
  { screen: "empty", label: "Empty" },
  { screen: "unread", label: "Unread" },
  { screen: "confirm", label: "Open elsewhere" },
  { screen: "today", label: "Today" },
];
const go = (over: Record<string, string>): void => {
  const next = new URLSearchParams(window.location.search);
  for (const [k, v] of Object.entries(over)) next.set(k, v);
  window.location.search = next.toString();
};

/** The prototype's own switcher, not a piece of the design: a screen per button, the theme, and how to reach the picker. */
function Bar() {
  const [shut, setShut] = useState(false);
  if (params.get("bar") === "0" || shut) return null;
  return (
    <div data-proto-bar className="fixed right-3 bottom-3 z-[200] flex max-w-[22rem] flex-col gap-2 rounded-xl border border-(--popover-edge) bg-popover p-3 text-xs text-muted-foreground shadow-(--dialog-shadow)">
      <p className="text-foreground">#2054 prototype</p>
      <p>Ctrl or Cmd+K, type resume, Enter. Or right-click wsp in the sidebar. Enter holds the pick on New thread, as main does today.</p>
      <div className="flex flex-wrap gap-1.5">
        {SCREENS.map(s => (
          <Button key={s.screen} size="xs" variant={s.screen === screen ? "default" : "outline"} onClick={() => go({ screen: s.screen })}>
            {s.label}
          </Button>
        ))}
      </div>
      <div className="flex flex-wrap gap-1.5">
        <Button size="xs" variant="outline" onClick={() => openPicker()}>
          Open the picker
        </Button>
        <Button size="xs" variant="outline" onClick={() => go({ theme: theme === "dark" ? "light" : "dark" })}>
          {theme === "dark" ? "Paper" : "Graphite"}
        </Button>
        <Button size="xs" variant="ghost" onClick={() => setShut(true)}>
          Hide
        </Button>
      </div>
    </div>
  );
}

function Openers() {
  useEffect(() => {
    if (screen === "palette") setTimeout(() => openCommandPalette({ query: "resume" }), 300);
    if (screen === "confirm") setTimeout(() => useConversationsStore.getState().ask({ project: "pr_wsp", row: WSP_ROWS[0]! }), 300);
    if (screen === "menu") {
      const when = (selector: string, then: (el: Element) => void, tries = 100): void => {
        const el = document.querySelector(selector);
        if (el !== null) then(el);
        else if (tries > 0) setTimeout(() => when(selector, then, tries - 1), 50);
      };
      if (window.innerWidth < 768) when("[data-sidebar=trigger]", el => (el as HTMLElement).click());
      when("[data-k=project-switcher]", el => {
        const r = el.getBoundingClientRect();
        el.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + 40, clientY: r.top + r.height / 2 }));
      });
    }
  }, []);
  return null;
}

createRoot(document.getElementById("root")!).render(
  <TooltipProvider>
    <AppShell>
      <Centre />
    </AppShell>
    <Picker />
    <Openers />
    <Bar />
  </TooltipProvider>,
);
